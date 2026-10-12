"""Checks a model rebuilt from OpenAI's checkpoint against a SHA-256, letting only the quantizer version stamp differ."""

import hashlib
import struct
import sys

# whisper.cpp header: 4-byte magic, then 11 int32 hparams; the last is ftype + 1000 * quantizer version.
STAMP_AT = 44


def main():
    path = sys.argv[1]
    want_sha = sys.argv[2]
    want_stamp = int(sys.argv[3])
    with open(path, "rb") as f:
        head = bytearray(f.read(STAMP_AT + 4))
        stamp = struct.unpack_from("<i", head, STAMP_AT)[0]
        # The pinned small and medium files carry quantizer version 1; their q5 blocks are the same as version 2.
        if stamp % 1000 != want_stamp % 1000:
            sys.exit(f"{path}: quantization type {stamp % 1000}, expected {want_stamp % 1000}")
        struct.pack_into("<i", head, STAMP_AT, want_stamp)
        digest = hashlib.sha256(head)
        while True:
            chunk = f.read(1 << 20)
            if not chunk:
                break
            digest.update(chunk)
    got = digest.hexdigest()
    if got != want_sha:
        sys.exit(f"{path}: sha256 {got}, expected {want_sha}")
    print(f"{path}: matches {want_sha} (stamp {stamp} compared as {want_stamp})")


if __name__ == "__main__":
    main()
