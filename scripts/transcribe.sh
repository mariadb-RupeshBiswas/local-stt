#!/bin/zsh
# Audio file (16 kHz WAV) to English text. Usage: ./transcribe.sh samples/hi.wav
M="${0:A:h:h}/models/ggml-large-v3-q5_0.bin"
exec whisper-cli -m "$M" -tr -l auto -nt -f "$1" 2>/dev/null
