// Thin C ABI over whisper.cpp so Rust never touches its large params struct; C++ only to stop exceptions at the boundary.
#include <cstddef>
#include <cstring>
#include "whisper.h"

static void lstt_quiet(enum ggml_log_level level, const char * text, void * user) {
    (void)level; (void)text; (void)user;
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

extern "C" int lstt_transcribe(void * ctx, const float * pcm, int n, int translate, const char * lang, int threads, char * out, int out_len) {
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
        if (whisper_full(wc, p, pcm, n) != 0) return -2;
        int segs = whisper_full_n_segments(wc);
        size_t used = 0;
        for (int i = 0; i < segs; i++) {
            const char * t = whisper_full_get_segment_text(wc, i);
            if (!t) continue;
            size_t len = std::strlen(t);
            // Segments do not always carry a leading space; without one, words fuse ("hereSo").
            if (used > 0 && len > 0 && out[used - 1] != ' ' && t[0] != ' ') {
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
