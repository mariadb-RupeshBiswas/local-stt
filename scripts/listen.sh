#!/bin/zsh
# Live mic to English text; Hindi gets translated. Ctrl+C stops.
M="${0:A:h:h}/models/ggml-large-v3-q5_0.bin"
exec whisper-stream -m "$M" -tr -l auto --step 0 --length 10000 -vth 0.6 "$@" 2>/dev/null
