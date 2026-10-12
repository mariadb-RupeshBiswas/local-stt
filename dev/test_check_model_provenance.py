"""Run: uv run python -I -m unittest discover -s dev -p 'test_*.py'"""

import hashlib
import os
import struct
import subprocess
import sys
import tempfile
import unittest

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "check-model-provenance.py")


def model_bytes(stamp, payload=b"weights"):
    return b"lmgg" + struct.pack("<10i", *range(10)) + struct.pack("<i", stamp) + payload


def check(data, want_sha, want_stamp):
    with tempfile.NamedTemporaryFile(delete=False) as f:
        f.write(data)
    try:
        run = subprocess.run([sys.executable, "-I", SCRIPT, f.name, want_sha, str(want_stamp)], capture_output=True)
        return run.returncode
    finally:
        os.unlink(f.name)


class CheckModelProvenance(unittest.TestCase):
    pinned = model_bytes(1009)
    pinned_sha = hashlib.sha256(pinned).hexdigest()

    def test_only_the_quantizer_version_may_differ(self):
        self.assertEqual(check(model_bytes(2009), self.pinned_sha, 1009), 0)

    def test_another_quantization_type_fails(self):
        self.assertNotEqual(check(model_bytes(2008), self.pinned_sha, 1009), 0)

    def test_different_weights_fail(self):
        self.assertNotEqual(check(model_bytes(2009, b"weightz"), self.pinned_sha, 1009), 0)


if __name__ == "__main__":
    unittest.main()
