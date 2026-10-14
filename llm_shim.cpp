// Thin C ABI over llama.cpp for note summaries; C++ only to stop exceptions at the boundary.
#include <atomic>
#include <cstring>
#include <string>
#include <vector>
#include "llama.h"

static void lstt_llm_quiet(enum ggml_log_level level, const char * text, void * user) {
    (void)level; (void)text; (void)user;
}

// Raised when the app quits, so a running summary stops at its next token.
static std::atomic<int> g_llm_abort{0};

extern "C" void lstt_llm_set_abort(int on) { g_llm_abort.store(on); }

extern "C" void * lstt_llm_load(const char * path, int gpu_layers) {
    try {
        llama_log_set(lstt_llm_quiet, nullptr);
        llama_backend_init();
        llama_model_params mp = llama_model_default_params();
        mp.n_gpu_layers = gpu_layers;
        return llama_model_load_from_file(path, mp);
    } catch (...) {
        return nullptr;
    }
}

extern "C" void lstt_llm_free(void * model) {
    try {
        llama_model_free(static_cast<llama_model *>(model));
    } catch (...) {
    }
}

// Writes the reply into out as UTF-8 with a closing NUL. Returns its length, -1 on failure,
// -2 when the prompt does not fit the context, -3 when out is too small, -4 when aborted.
extern "C" int lstt_llm_generate(void * model_ptr, const char * prompt, const char * grammar, const char * root,
                                 int n_ctx, int max_tokens, int threads, char * out, int out_len) {
    llama_context * ctx = nullptr;
    llama_sampler * chain = nullptr;
    int result = -1;
    try {
        auto * model = static_cast<llama_model *>(model_ptr);
        const llama_vocab * vocab = llama_model_get_vocab(model);
        int len = (int)std::strlen(prompt);
        std::vector<llama_token> tokens(len + 16);
        int n = llama_tokenize(vocab, prompt, len, tokens.data(), (int)tokens.size(), true, true);
        if (n < 0) return -1;
        tokens.resize(n);
        if (n + max_tokens > n_ctx) return -2;

        llama_context_params cp = llama_context_default_params();
        cp.n_ctx = n_ctx;
        cp.n_batch = 512;
        cp.n_threads = threads;
        cp.n_threads_batch = threads;
        ctx = llama_init_from_model(model, cp);
        if (!ctx) return -1;

        chain = llama_sampler_chain_init(llama_sampler_chain_default_params());
        llama_sampler * g = llama_sampler_init_grammar(vocab, grammar, root);
        if (!g) {
            llama_sampler_free(chain);
            llama_free(ctx);
            return -1;
        }
        llama_sampler_chain_add(chain, g);
        llama_sampler_chain_add(chain, llama_sampler_init_greedy());

        for (int i = 0; i < n; i += 512) {
            int count = n - i < 512 ? n - i : 512;
            if (llama_decode(ctx, llama_batch_get_one(tokens.data() + i, count)) != 0) throw 0;
        }
        std::string text;
        for (int i = 0; i < max_tokens; i++) {
            if (g_llm_abort.load()) {
                result = -4;
                break;
            }
            llama_token t = llama_sampler_sample(chain, ctx, -1);
            if (llama_vocab_is_eog(vocab, t)) break;
            char piece[256];
            int k = llama_token_to_piece(vocab, t, piece, sizeof(piece), 0, false);
            if (k > 0) text.append(piece, k);
            if (llama_decode(ctx, llama_batch_get_one(&t, 1)) != 0) throw 0;
        }
        if (result != -4) {
            if ((int)text.size() + 1 > out_len) {
                result = -3;
            } else {
                std::memcpy(out, text.data(), text.size());
                out[text.size()] = '\0';
                result = (int)text.size();
            }
        }
    } catch (...) {
        result = -1;
    }
    if (chain) llama_sampler_free(chain);
    if (ctx) llama_free(ctx);
    return result;
}
