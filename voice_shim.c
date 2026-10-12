// Flat functions over sherpa-onnx's speaker embedding C API, so Rust never mirrors its structs.
#include <string.h>
#include "c-api.h"

const SherpaOnnxSpeakerEmbeddingExtractor *lstt_voice_new(const char *model, int threads) {
  SherpaOnnxSpeakerEmbeddingExtractorConfig config;
  memset(&config, 0, sizeof config);
  config.model = model;
  config.num_threads = threads;
  config.provider = "cpu";
  return SherpaOnnxCreateSpeakerEmbeddingExtractor(&config);
}

int lstt_voice_dim(const SherpaOnnxSpeakerEmbeddingExtractor *ex) {
  return SherpaOnnxSpeakerEmbeddingExtractorDim(ex);
}

// Writes the voice fingerprint of 16 kHz mono samples into out (dim floats); -1 when the audio is too short.
int lstt_voice_embed(const SherpaOnnxSpeakerEmbeddingExtractor *ex, const float *samples, int n, float *out, int dim) {
  const SherpaOnnxOnlineStream *stream = SherpaOnnxSpeakerEmbeddingExtractorCreateStream(ex);
  if (stream == NULL) return -1;
  SherpaOnnxOnlineStreamAcceptWaveform(stream, 16000, samples, n);
  SherpaOnnxOnlineStreamInputFinished(stream);
  int rc = -1;
  if (SherpaOnnxSpeakerEmbeddingExtractorIsReady(ex, stream)) {
    const float *v = SherpaOnnxSpeakerEmbeddingExtractorComputeEmbedding(ex, stream);
    if (v != NULL) {
      memcpy(out, v, sizeof(float) * dim);
      SherpaOnnxSpeakerEmbeddingExtractorDestroyEmbedding(v);
      rc = 0;
    }
  }
  SherpaOnnxDestroyOnlineStream(stream);
  return rc;
}

void lstt_voice_free(const SherpaOnnxSpeakerEmbeddingExtractor *ex) {
  if (ex != NULL) SherpaOnnxDestroySpeakerEmbeddingExtractor(ex);
}
