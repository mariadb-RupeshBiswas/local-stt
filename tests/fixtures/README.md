# Test fixtures

`voices-arctic.json`: voice fingerprints (192 numbers each, rounded to 4 decimals) of four speakers
from the CMU ARCTIC databases (Carnegie Mellon University), the first six utterances of speakers
aup, eey, fem and ljm as published at https://huggingface.co/datasets/MikhailT/cmu-arctic, made with
the CAM++ zh_en model that `src/notes/voices.rs` pins. Numbers only; no audio is stored here.
