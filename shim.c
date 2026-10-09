/* Thin C layer over whisper.cpp so Rust never touches its large params struct. Owned by Task 2. */
#include <stddef.h>
#include "whisper.h"

void * lstt_load(const char * path) { (void)path; return NULL; }

int lstt_transcribe(void * ctx, const float * pcm, int n, int translate, const char * lang, int threads, char * out, int out_len) {
    (void)ctx; (void)pcm; (void)n; (void)translate; (void)lang; (void)threads; (void)out; (void)out_len;
    return -1;
}

void lstt_free(void * ctx) { (void)ctx; }
