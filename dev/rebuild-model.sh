#!/bin/sh
# Rebuilds one catalog model from OpenAI's own checkpoint and checks it against src/models.rs.
# Usage: dev/rebuild-model.sh small|medium|large-v3 <empty work dir> <whisper-quantize binary>
set -eu
model="$1"
work="$2"
quantize="$3"
root="$(cd "$(dirname "$0")/.." && pwd)"

# OpenAI's URLs carry the checkpoint's SHA-256; the assets come from openai/whisper at a fixed commit.
case "$model" in
  small)
    pt_sha=9ecf779972d90ba49c06d968637d720dd632c55bbf19d441fb42bf17a411e794
    file=ggml-small-q5_1.bin qtype=q5_1 stamp=1009
    assets=55237228425e39828bbb964fd7bf774c9962eb67 ;;
  medium)
    pt_sha=345ae4da62f9b3d59415adc60127b97c714f32e89e936602e85993674d08dcb1
    file=ggml-medium-q5_0.bin qtype=q5_0 stamp=1008
    assets=55237228425e39828bbb964fd7bf774c9962eb67 ;;
  large-v3)
    pt_sha=e5b1a55b89c1367dacf97e3e19bfd829a01529dbfdeefa8caeb59b3f1b81dadb
    file=ggml-large-v3-q5_0.bin qtype=q5_0 stamp=2008
    # The 128-band mel filters for large-v3 arrived with this commit; it also flips one zero's sign in the 80-band set.
    assets=c5d42560760a05584c1c79546a098287e5a771eb ;;
  *)
    echo "unknown model: $model" >&2
    exit 1 ;;
esac

want_sha="$(grep -A 2 "file: \"$file\"" "$root/src/models.rs" | sed -n 's/.*sha256: "\([0-9a-f]*\)".*/\1/p')"
[ -n "$want_sha" ] || { echo "$file is not in src/models.rs" >&2; exit 1; }

mkdir -p "$work/whisper/whisper/assets" "$work/out"
curl -fsSL -o "$work/$model.pt" "https://openaipublic.azureedge.net/main/whisper/models/$pt_sha/$model.pt"
got_sha="$(shasum -a 256 "$work/$model.pt" | cut -d ' ' -f 1)"
# A .pt file is a pickle, so it is loaded only once it is exactly the file OpenAI published.
[ "$got_sha" = "$pt_sha" ] || { echo "$model.pt sha256 $got_sha, expected $pt_sha" >&2; exit 1; }
for asset in mel_filters.npz multilingual.tiktoken; do
  curl -fsSL -o "$work/whisper/whisper/assets/$asset" "https://raw.githubusercontent.com/openai/whisper/$assets/whisper/assets/$asset"
done

uv run --no-project --python 3.12 --with torch==2.14.1 --with numpy==2.5.2 \
  python -I "$root/vendor/whisper.cpp/models/convert-pt-to-ggml.py" "$work/$model.pt" "$work/whisper" "$work/out"
rm -f "$work/$model.pt"
"$quantize" "$work/out/ggml-model.bin" "$work/out/$file" "$qtype"
rm -f "$work/out/ggml-model.bin"
uv run --no-project --python 3.12 python -I "$root/dev/check-model-provenance.py" "$work/out/$file" "$want_sha" "$stamp"
