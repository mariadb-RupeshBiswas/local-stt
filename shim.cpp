// Thin C ABI over whisper.cpp so Rust never touches its large params struct; C++ only to stop exceptions at the boundary.
#include <atomic>
#include <cstddef>
#include <cstring>
#include "whisper.h"

static void lstt_quiet(enum ggml_log_level level, const char * text, void * user) {
    (void)level; (void)text; (void)user;
}

// Raised when a final transcription is queued or the app quits, so a live preview pass stops early.
static std::atomic<int> g_abort{0};

static bool lstt_should_abort(void * user) {
    (void)user;
    return g_abort.load() != 0;
}

extern "C" void lstt_set_abort(int on) { g_abort.store(on); }

// Scripts written without spaces between words; joining their segments must not add any.
static bool lstt_spaceless(const char * lang) {
    static const char * const langs[] = {"zh", "ja", "th", "lo", "my", "km", "yue", nullptr};
    for (int i = 0; lang && langs[i]; i++) {
        if (std::strcmp(lang, langs[i]) == 0) return true;
    }
    return false;
}

extern "C" void * lstt_load(const char * path) {
    try {
        whisper_log_set(lstt_quiet, nullptr);
        struct whisper_context_params cp = whisper_context_default_params();
        return whisper_init_from_file_with_params(path, cp);
    } catch (...) {
        return nullptr;
    }
}

extern "C" int lstt_transcribe(void * ctx, const float * pcm, int n, int translate, const char * lang, int threads, int abortable, char * out, int out_len) {
    if (!ctx || !out || out_len <= 0) return -1;
    out[0] = '\0';
    if (n <= 0) return 0;
    try {
        auto * wc = static_cast<struct whisper_context *>(ctx);
        struct whisper_full_params p = whisper_full_default_params(WHISPER_SAMPLING_GREEDY);
        p.translate = translate != 0;
        p.language = (lang && lang[0]) ? lang : "auto";
        p.detect_language = false;
        p.n_threads = threads > 0 ? threads : 4;
        p.print_progress = false;
        p.print_realtime = false;
        p.print_timestamps = false;
        p.print_special = false;
        p.no_timestamps = true;
        p.single_segment = false;
        p.suppress_blank = true;
        if (abortable) {
            p.abort_callback = lstt_should_abort;
            p.abort_callback_user_data = nullptr;
        }
        if (whisper_full(wc, p, pcm, n) != 0) return abortable && g_abort.load() ? -5 : -2;
        const char * out_lang = translate ? "en" : whisper_lang_str(whisper_full_lang_id(wc));
        const bool spaceless = lstt_spaceless(out_lang);
        int segs = whisper_full_n_segments(wc);
        size_t used = 0;
        for (int i = 0; i < segs; i++) {
            const char * t = whisper_full_get_segment_text(wc, i);
            if (!t) continue;
            size_t len = std::strlen(t);
            // Segments do not always carry a leading space; without one, words fuse ("hereSo").
            if (!spaceless && used > 0 && len > 0 && out[used - 1] != ' ' && t[0] != ' ') {
                if (used + 2 > static_cast<size_t>(out_len)) return -3;
                out[used++] = ' ';
            }
            if (used + len + 1 > static_cast<size_t>(out_len)) return -3;
            std::memcpy(out + used, t, len);
            used += len;
        }
        out[used] = '\0';
        return 0;
    } catch (...) {
        out[0] = '\0';
        return -4;
    }
}

extern "C" void lstt_free(void * ctx) {
    if (ctx) whisper_free(static_cast<struct whisper_context *>(ctx));
}

// Points this process's error output (fd 2 and C stderr) at a log file, so a native abort's reason is kept.
#ifdef _WIN32
#include <cstdio>
#include <fcntl.h>
#include <io.h>
#include <share.h>
#include <sys/stat.h>
#include <windows.h>
extern "C" int lstt_redirect_stderr(const wchar_t * path) {
    int fd = -1;
    // Shared, so the report can read the file and Off can point stderr elsewhere while the app runs.
    if (_wsopen_s(&fd, path, _O_WRONLY | _O_CREAT | _O_APPEND | _O_BINARY, _SH_DENYNO, _S_IREAD | _S_IWRITE) != 0) return -1;
    FILE * f = nullptr;
    // A GUI process may start with no stderr stream; NUL gives it one on fd 2 to point at the file.
    if (_wfreopen_s(&f, L"NUL", L"ab", stderr) != 0 || f == nullptr) { _close(fd); return -1; }
    int ok = _dup2(fd, _fileno(stderr));
    _close(fd);
    if (ok != 0) return -1;
    setvbuf(stderr, nullptr, _IONBF, 0);
    SetStdHandle(STD_ERROR_HANDLE, (HANDLE) _get_osfhandle(_fileno(stderr)));
    return 0;
}
#else
#include <fcntl.h>
#include <unistd.h>
extern "C" int lstt_redirect_stderr(const char * path) {
    int fd = open(path, O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0600);
    if (fd < 0) return -1;
    int ok = dup2(fd, 2);
    close(fd);
    return ok < 0 ? -1 : 0;
}
#endif
