# Test fixtures

`voices-arctic.json`: voice fingerprints (192 numbers each, rounded to 4 decimals) of four speakers
from the CMU ARCTIC databases (Carnegie Mellon University), the first six utterances of speakers
aup, eey, fem and ljm as published at https://huggingface.co/datasets/MikhailT/cmu-arctic, made with
the CAM++ zh_en model that `src/notes/voices.rs` pins. Numbers only; no audio is stored here.

`voices-call.json`: the same speakers aup, eey and ljm cut into 120 call-like lines (seed 7): 55% are
1 to 3 s windows holding a 0.3 to 1.2 s snippet in near-silence, like a "yeah", the rest 3 to 20 s
of speech, each at a random volume and passed through AAC at 24 kbps as a stand-in for a call
codec. `samples` is each line's length at 16 kHz. Fingerprints made as above; numbers only.
