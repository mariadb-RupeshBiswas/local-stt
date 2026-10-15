# Third-party notices

local-stt is released under the MIT License (see [LICENSE](LICENSE)). The app and the Python wheel that carries it include or link the third-party software listed below. Each component keeps its own licence; the required notices are reproduced here.

Models are not part of the wheel. The app downloads them when you choose them, and they are listed in section 4 for reference only.

Sections:

1. Native code built into the binary
2. Rust crates
3. Licence texts
4. Models downloaded at run time

## 1. Native code built into the binary

| Component | Licence | Source |
|---|---|---|
| whisper.cpp (vendored in `vendor/whisper.cpp`, includes ggml) | MIT | https://github.com/ggml-org/whisper.cpp |
| llama.cpp, libllama only (vendored in `vendor/llama.cpp`) | MIT | https://github.com/ggml-org/llama.cpp |
| sherpa-onnx 1.13.8 (prebuilt static libraries, macOS arm64 and Windows x64 only) | Apache-2.0 | https://github.com/k2-fsa/sherpa-onnx |
| ONNX Runtime (static, inside the sherpa-onnx archive) | MIT | https://github.com/microsoft/onnxruntime |
| kaldi-native-fbank, kaldi-decoder, OpenFst and kaldifst forks, ssentencepiece (inside the sherpa-onnx archive) | Apache-2.0 | https://github.com/k2-fsa/sherpa-onnx |
| KISS FFT (inside the sherpa-onnx archive) | BSD-3-Clause | https://github.com/mborgerding/kissfft |
| sonora 0.2.0 and sonora-* crates (echo cancellation, a Rust port of WebRTC AEC3) | BSD-3-Clause | https://github.com/dignifiedquire/sonora |

Notes:

- whisper.cpp and llama.cpp share one copy of ggml. Their LICENSE files are identical: "Copyright (c) 2023-2026 The ggml authors".
- The sherpa-onnx archive is downloaded by `build.rs` at build time (checksum pinned) and contains no licence files. The bundled components and their licences above are taken from the upstream projects, and ONNX Runtime's own third-party list is at https://github.com/microsoft/onnxruntime/blob/main/ThirdPartyNotices.txt.
- sherpa-onnx's licence file is also in `vendor/sherpa-onnx/LICENSE`, next to the C header the app compiles against.
- Platform libraries (Apple Metal, Accelerate and other system frameworks, the Windows runtime) are used through the operating system and are not redistributed.

## 2. Rust crates

All normal (non-dev, non-build) dependencies of the `local-stt` package on `aarch64-apple-darwin` and `x86_64-pc-windows-msvc`. Where a crate offers a choice (`OR`), the MIT option is relied on if present. The licence field is each crate's own `license` metadata.


### MIT OR Apache-2.0 (127)

anyhow 1.0.104, arboard 3.6.1, base64 0.21.7, base64 0.22.1, base64 0.23.1, bitflags 2.13.2, block-buffer 0.10.4, block-buffer 0.12.1, camino 1.2.6, cargo-platform 0.1.9, cfg-if 1.0.5, cookie 0.18.2, core-foundation 0.10.1, core-foundation-sys 0.8.7, core-graphics 0.25.0, core-graphics-types 0.2.0, cpufeatures 0.2.17, cpufeatures 0.3.1, crc32fast 1.5.2, crossbeam-channel 0.5.17, crossbeam-utils 0.8.23, crypto-common 0.1.7, crypto-common 0.2.2, dasp_sample 0.11.0, deranged 0.5.8, digest 0.10.7, digest 0.11.3, dirs 7.0.0, dirs-sys 0.5.0, displaydoc 0.2.7, dtoa 1.0.11, dyn-clone 1.0.20, embed_plist 1.2.2, erased-serde 0.4.10, fdeflate 0.3.7, flate2 1.1.10, form_urlencoded 1.2.2, getrandom 0.3.4, getrandom 0.4.3, glob 0.3.4, hashbrown 0.12.3, hashbrown 0.17.1, heck 0.5.0, html5ever 0.39.0, http 1.5.0, hybrid-array 0.4.15, idna 1.1.0, itoa 1.0.18, jsonptr 0.7.1, keyboard-types 0.8.3, libc 0.2.190, lock_api 0.4.14, log 0.4.34, markup5ever 0.39.0, mime 0.3.17, num-conv 0.2.2, once_cell 1.21.4, parking_lot 0.12.5, parking_lot_core 0.9.12, percent-encoding 2.3.2, png 0.17.16, png 0.18.1, powerfmt 0.2.1, proc-macro2 1.0.107, quote 1.0.47, regex 1.13.1, regex-automata 0.4.18, regex-syntax 0.8.11, scopeguard 1.2.0, semver 1.0.28, serde 1.0.229, serde-untagged 0.1.9, serde_core 1.0.229, serde_derive 1.0.229, serde_derive_internals 0.29.1, serde_json 1.0.151, serde_repr 0.1.21, serde_spanned 1.1.2, serde_with 3.24.0, serde_with_macros 3.24.0, serialize-to-javascript 0.1.2, serialize-to-javascript-impl 0.1.2, servo_arc 0.4.3, sha2 0.10.9, sha2 0.11.0, siphasher 1.0.4, smallvec 1.16.2, softbuffer 0.4.8, stable_deref_trait 1.2.1, string_cache 0.9.0, swift-rs 1.0.8, syn 2.0.119, syn 3.0.6, tendril 0.5.1, thiserror 2.0.21, thiserror-impl 2.0.21, time 0.3.55, time-core 0.1.9, time-macros 0.2.32, toml 1.1.8+spec-1.1.0, toml_datetime 1.1.2+spec-1.1.0, toml_parser 1.1.5+spec-1.1.0, toml_writer 1.1.3+spec-1.1.0, tray-icon 0.25.1, typeid 1.0.3, typenum 1.20.1, unicode-segmentation 1.13.3, unicode-xid 0.2.6, url 2.5.8, web-time 1.1.0, web_atoms 0.2.6, windows 0.62.2, windows-collections 0.3.2, windows-core 0.62.2, windows-future 0.3.2, windows-implement 0.60.2, windows-interface 0.59.3, windows-link 0.2.1, windows-numerics 0.3.1, windows-result 0.4.1, windows-strings 0.5.1, windows-sys 0.60.2, windows-sys 0.61.2, windows-targets 0.53.5, windows-threading 0.2.1, windows-version 0.1.7, windows_x86_64_msvc 0.53.1

### MIT (39)

block2 0.6.2, bytes 1.12.1, cargo_metadata 0.19.2, cfb 0.14.0, darling 0.24.1, darling_core 0.24.1, darling_macro 0.24.1, derive_more 2.1.1, derive_more-impl 2.1.1, dom_query 0.28.0, generic-array 0.14.7, ico 0.5.0, infer 0.22.0, new_debug_unreachable 1.0.6, objc2 0.6.5, objc2-encode 4.1.0, objc2-foundation 0.3.2, phf 0.13.1, phf_generator 0.13.1, phf_macros 0.13.1, phf_shared 0.13.1, plist 1.10.1, precomputed-hash 0.1.1, quick-xml 0.42.0, schemars 0.8.22, schemars_derive 0.8.22, simd-adler32 0.3.10, strsim 0.11.1, synstructure 0.14.0, tokio 1.53.2, tracing 0.1.44, tracing-attributes 0.1.31, tracing-core 0.1.36, urlpattern 0.6.0, webview2-com 0.39.1, webview2-com-macros 0.8.1, webview2-com-sys 0.39.1, winnow 1.0.4, zmij 1.0.23

### Apache-2.0 OR MIT (22)

bit-set 0.8.0, bit-vec 0.8.0, const-oid 0.10.2, ctor 1.0.13, equivalent 1.0.2, fastrand 2.5.0, idna_adapter 1.2.2, indexmap 1.9.3, indexmap 2.14.2, muda 0.20.0, pin-project-lite 0.2.17, rustc-hash 2.1.3, tauri 2.12.2, tauri-codegen 2.7.1, tauri-macros 2.7.1, tauri-runtime 2.12.1, tauri-runtime-wry 2.12.1, tauri-utils 2.10.1, utf8_iter 1.0.4, uuid 1.27.0, window-vibrancy 0.8.1, wry 0.57.0

### Unicode-3.0 (18)

icu_collections 2.3.0, icu_locale_core 2.3.0, icu_normalizer 2.3.0, icu_normalizer_data 2.3.0, icu_properties 2.3.0, icu_properties_data 2.3.0, icu_provider 2.3.1, litemap 0.8.3, potential_utf 0.1.6, tinystr 0.8.4, writeable 0.6.4, yoke 0.8.3, yoke-derive 0.8.4, zerofrom 0.1.8, zerofrom-derive 0.1.8, zerotrie 0.2.5, zerovec 0.11.8, zerovec-derive 0.11.6

### Zlib OR Apache-2.0 OR MIT (12)

bytemuck 1.25.2, bytemuck_derive 1.12.1, dispatch2 0.3.1, objc2-app-kit 0.3.2, objc2-audio-toolbox 0.3.2, objc2-core-audio 0.3.2, objc2-core-audio-types 0.3.2, objc2-core-foundation 0.3.2, objc2-core-graphics 0.3.2, objc2-exception-helper 0.1.1, objc2-quartz-core 0.3.2, objc2-web-kit 0.3.2

### BSD-3-Clause (9)

alloc-no-stdlib 3.0.0, alloc-stdlib 0.3.0, sonora 0.2.0, sonora-aec3 0.2.0, sonora-agc2 0.2.0, sonora-common-audio 0.2.0, sonora-fft 0.2.0, sonora-ns 0.2.0, sonora-simd 0.2.0

### MIT/Apache-2.0 (7)

bitflags 1.3.2, coreaudio-rs 0.14.2, foreign-types 0.5.0, foreign-types-macros 0.2.4, foreign-types-shared 0.3.1, ident_case 1.0.1, json-patch 4.2.0

### MPL-2.0 (5)

cssparser 0.37.0, cssparser-macros 0.7.1, dtoa-short 0.3.5, option-ext 0.2.0, selectors 0.38.0

### Unlicense OR MIT (4)

aho-corasick 1.1.5, byteorder 1.5.0, memchr 2.8.3, winapi-util 0.1.11

### Apache-2.0 (2)

cpal 0.18.2, tao 0.37.1

### BSL-1.0 (2)

clipboard-win 5.4.1, error-code 3.4.0

### MIT OR Zlib OR Apache-2.0 (2)

miniz_oxide 0.8.9, miniz_oxide 0.9.1

### Unlicense/MIT (2)

same-file 1.0.6, walkdir 2.5.0

### (MIT OR Apache-2.0) AND Unicode-3.0 (1)

unicode-ident 1.0.26

### 0BSD OR MIT OR Apache-2.0 (1)

adler2 2.0.1

### Apache-2.0 / MIT (1)

fnv 1.0.7

### Apache-2.0 AND MIT (1)

dpi 0.1.2

### BSD-2-Clause OR MIT OR Apache-2.0 (1)

mach2 0.6.0

### BSD-3-Clause AND MIT (1)

brotli 9.0.0

### BSD-3-Clause/MIT (1)

brotli-decompressor 6.0.1

### CC0-1.0 OR MIT-0 OR Apache-2.0 (1)

dunce 1.0.5

### MIT OR Apache-2.0 OR Zlib (1)

raw-window-handle 0.6.2

### Zlib (1)

foldhash 0.2.0

Total: 261 crates.

### Copyright notices of the crates

Taken from the crates' licence files; where a crate ships none, the `authors` field of its Cargo.toml is shown, and where that is empty, the repository URL.

- adler2 2.0.1: Jonas Schievink <jonasschievink@gmail.com>, oyvindln <oyvindln@users.noreply.github.com>
- aho-corasick 1.1.5: Copyright (c) 2015 Andrew Gallant
- alloc-no-stdlib 3.0.0: Copyright (c) 2016 Dropbox, Inc.
- alloc-stdlib 0.3.0: Daniel Reiter Horn <danielrh@dropbox.com>
- anyhow 1.0.104: David Tolnay <dtolnay@gmail.com>
- arboard 3.6.1: Copyright (c) 2022 The Arboard contributors
- base64 0.21.7: Copyright (c) 2015 Alice Maz
- base64 0.22.1: Copyright (c) 2015 Alice Maz
- base64 0.23.1: Copyright (c) 2025 Alice Maz, Marshall Pierce
- bit-set 0.8.0: Copyright (c) 2023 The Rust Project Developers
- bit-vec 0.8.0: Copyright (c) 2023 The Rust Project Developers
- bitflags 1.3.2: Copyright (c) 2014 The Rust Project Developers
- bitflags 2.13.2: Copyright (c) 2014 The Rust Project Developers
- block-buffer 0.10.4: Copyright (c) 2018-2019 The RustCrypto Project Developers
- block-buffer 0.12.1: Copyright (c) 2018-2025 The RustCrypto Project Developers
- block2 0.6.2: Mads Marquart <mads@marquart.dk>
- brotli 9.0.0: Copyright (c) 2016 Dropbox, Inc. / Copyright (c) 2009, 2010, 2013-2016 by the Brotli Authors.
- brotli-decompressor 6.0.1: Copyright (c) 2016 Dropbox, Inc.
- bytemuck 1.25.2: Copyright (c) 2019 Daniel "Lokathor" Gee.
- bytemuck_derive 1.12.1: Copyright (c) 2019 Daniel "Lokathor" Gee.
- byteorder 1.5.0: Copyright (c) 2015 Andrew Gallant
- bytes 1.12.1: Copyright (c) 2018 Carl Lerche
- camino 1.2.6: Without Boats <saoirse@without.boats>, Ashley Williams <ashley666ashley@gmail.com>, Steve Klabnik <steve@steveklabnik.com>, Rain <rain@sunshowers.io>
- cargo-platform 0.1.9: https://github.com/rust-lang/cargo
- cargo_metadata 0.19.2: Oliver Schneider <git-spam-no-reply9815368754983@oli-obk.de>
- cfb 0.14.0: Copyright (c) 2017 Matthew D. Steele
- cfg-if 1.0.5: Copyright (c) 2014 Alex Crichton
- clipboard-win 5.4.1: Douman <douman@gmx.se>
- const-oid 0.10.2: Copyright (c) 2020-2026 The RustCrypto Project Developers
- cookie 0.18.2: Copyright 2017 Sergio Benitez / Copyright 2014 Alex Chricton / Copyright (c) 2017 Sergio Benitez / Copyright (c) 2014 Alex Crichton
- core-foundation 0.10.1: Copyright (c) 2012-2013 Mozilla Foundation
- core-foundation-sys 0.8.7: Copyright (c) 2012-2013 Mozilla Foundation
- core-graphics 0.25.0: Copyright (c) 2012-2013 Mozilla Foundation
- core-graphics-types 0.2.0: Copyright (c) 2012-2013 Mozilla Foundation
- coreaudio-rs 0.14.2: Copyright (c) 2015
- cpal 0.18.2: https://github.com/RustAudio/cpal
- cpufeatures 0.2.17: Copyright (c) 2020-2025 The RustCrypto Project Developers
- cpufeatures 0.3.1: Copyright (c) 2020-2026 The RustCrypto Project Developers
- crc32fast 1.5.2: Copyright (c) 2018 Sam Rijs, Alex Crichton and contributors
- crossbeam-channel 0.5.17: Copyright (c) 2019 The Crossbeam Project Developers / Copyright (c) 2009 The Go Authors. All rights reserved.
- crossbeam-utils 0.8.23: Copyright (c) 2019 The Crossbeam Project Developers
- crypto-common 0.1.7: Copyright (c) 2021 RustCrypto Developers
- crypto-common 0.2.2: Copyright (c) 2021-2026 RustCrypto Developers
- cssparser 0.37.0: (c) under Patent Claims infringed by Covered Software in the absence of
- cssparser-macros 0.7.1: (c) under Patent Claims infringed by Covered Software in the absence of
- ctor 1.0.13: Matt Mastracci <matthew@mastracci.com>
- darling 0.24.1: Copyright (c) 2017 Ted Driggs
- darling_core 0.24.1: Copyright (c) 2017 Ted Driggs
- darling_macro 0.24.1: Copyright (c) 2017 Ted Driggs
- dasp_sample 0.11.0: mitchmindtree <mitchell.nordine@gmail.com>
- deranged 0.5.8: Copyright 2024 Jacob Pratt et al. / Copyright (c) 2024 Jacob Pratt et al.
- derive_more 2.1.1: Copyright (c) 2016 Jelte Fennema
- derive_more-impl 2.1.1: Copyright (c) 2016 Jelte Fennema
- digest 0.10.7: Copyright (c) 2017 Artyom Pavlov
- digest 0.11.3: Copyright (c) 2017-2025 RustCrypto Developers / Copyright (c) 2017 Artyom Pavlov
- dirs 7.0.0: Copyright (c) 2018-2019 dirs-rs contributors
- dirs-sys 0.5.0: Copyright (c) 2018-2019 dirs-rs contributors
- dispatch2 0.3.1: Mads Marquart <mads@marquart.dk>, Mary <mary@mary.zone>
- displaydoc 0.2.7: Jane Lusby <jlusby@yaah.dev>
- dom_query 0.28.0: Copyright (c) 2023 Mykola Humanov
- dpi 0.1.2: Copyright (c) 2018 Jorge Aparicio / Copyright © 2005-2020 Rich Felker, et al. / Copyright © 1993,2004 Sun Microsystems or / Copyright © 2003-2011 David Schultz or
- dtoa 1.0.11: David Tolnay <dtolnay@gmail.com>
- dtoa-short 0.3.5: (c) under Patent Claims infringed by Covered Software in the absence of
- dunce 1.0.5: Kornel <kornel@geekhood.net>
- dyn-clone 1.0.20: David Tolnay <dtolnay@gmail.com>
- embed_plist 1.2.2: Copyright (c) 2020 Nikolai Vazquez
- equivalent 1.0.2: Copyright (c) 2016--2023
- erased-serde 0.4.10: David Tolnay <dtolnay@gmail.com>
- error-code 3.4.0: Douman <douman@gmx.se>
- fastrand 2.5.0: Stjepan Glavina <stjepang@gmail.com>
- fdeflate 0.3.7: The image-rs Developers
- flate2 1.1.10: Copyright (c) 2014-2026 Alex Crichton
- fnv 1.0.7: Copyright (c) 2017 Contributors
- foldhash 0.2.0: Copyright (c) 2024 Orson Peters
- foreign-types 0.5.0: Copyright (c) 2017 The foreign-types Developers
- foreign-types-macros 0.2.4: Copyright (c) 2017 The foreign-types Developers
- foreign-types-shared 0.3.1: Copyright (c) 2017 The foreign-types Developers
- form_urlencoded 1.2.2: Copyright (c) 2013-2016 The rust-url developers
- generic-array 0.14.7: Copyright (c) 2015 Bartłomiej Kamiński
- getrandom 0.3.4: Copyright (c) 2018-2025 The rust-random Project Developers / Copyright (c) 2014 The Rust Project Developers
- getrandom 0.4.3: Copyright (c) 2018-2026 The rust-random Project Developers / Copyright (c) 2014 The Rust Project Developers
- glob 0.3.4: Copyright (c) 2014 The Rust Project Developers
- hashbrown 0.12.3: Copyright (c) 2016 Amanieu d'Antras
- hashbrown 0.17.1: Copyright (c) 2016 Amanieu d'Antras
- heck 0.5.0: Copyright (c) 2015 The Rust Project Developers
- html5ever 0.39.0: Copyright (c) 2014 The html5ever Project Developers
- http 1.5.0: Copyright 2017 http-rs authors / Copyright (c) 2017 http-rs authors
- hybrid-array 0.4.15: Copyright (c) 2022-2026 The RustCrypto Project Developers
- ico 0.5.0: Copyright (c) 2018 Matthew D. Steele
- icu_collections 2.3.0: Copyright © 2020-2024 Unicode, Inc.
- icu_locale_core 2.3.0: Copyright © 2020-2024 Unicode, Inc.
- icu_normalizer 2.3.0: Copyright © 2020-2024 Unicode, Inc.
- icu_normalizer_data 2.3.0: Copyright © 2020-2024 Unicode, Inc.
- icu_properties 2.3.0: Copyright © 2020-2024 Unicode, Inc.
- icu_properties_data 2.3.0: Copyright © 2020-2024 Unicode, Inc.
- icu_provider 2.3.1: Copyright © 2020-2024 Unicode, Inc.
- ident_case 1.0.1: Ted Driggs <ted.driggs@outlook.com>
- idna 1.1.0: Copyright (c) 2013-2025 The rust-url developers
- idna_adapter 1.2.2: The rust-url developers
- indexmap 1.9.3: Copyright (c) 2016--2017
- indexmap 2.14.2: Copyright (c) 2016--2017
- infer 0.22.0: Copyright (c) 2019 Bojan
- itoa 1.0.18: David Tolnay <dtolnay@gmail.com>
- json-patch 4.2.0: Copyright (c) 2017 Ivan Dubrov
- jsonptr 0.7.1: Copyright 2024 Chance Dinkins / Copyright (c) 2022 Chance Dinkins
- keyboard-types 0.8.3: Copyright (c) 2017 Pyfisch
- libc 0.2.190: https://github.com/rust-lang/libc
- litemap 0.8.3: Copyright © 2020-2024 Unicode, Inc.
- lock_api 0.4.14: Copyright (c) 2016 The Rust Project Developers
- log 0.4.34: Copyright (c) 2014 The Rust Project Developers
- mach2 0.6.0: Copyright (c) 2019 Nick Fitzgerald, 2021 Yuki Okushi
- markup5ever 0.39.0: Copyright (c) 2014 The html5ever Project Developers
- memchr 2.8.3: Copyright (c) 2015 Andrew Gallant
- mime 0.3.17: Copyright (c) 2014 Sean McArthur
- miniz_oxide 0.8.9: Copyright 2013-2014 RAD Game Tools and Valve Software / Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC / Copyright (c) 2017 Frommi / Copyright (c) 2017-2024 oyvindln
- miniz_oxide 0.9.1: Copyright 2013-2014 RAD Game Tools and Valve Software / Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC / Copyright (c) 2017 Frommi / Copyright (c) 2017-2024 oyvindln
- muda 0.20.0: Copyright (c) 2022-2022 Tauri Programme within The Commons Conservancy
- new_debug_unreachable 1.0.6: Copyright (c) 2015 Jonathan Reem
- num-conv 0.2.2: Jacob Pratt <jacob@jhpratt.dev>
- objc2 0.6.5: Mads Marquart <mads@marquart.dk>
- objc2-app-kit 0.3.2: https://github.com/madsmtm/objc2
- objc2-audio-toolbox 0.3.2: https://github.com/madsmtm/objc2
- objc2-core-audio 0.3.2: https://github.com/madsmtm/objc2
- objc2-core-audio-types 0.3.2: https://github.com/madsmtm/objc2
- objc2-core-foundation 0.3.2: https://github.com/madsmtm/objc2
- objc2-core-graphics 0.3.2: https://github.com/madsmtm/objc2
- objc2-encode 4.1.0: Mads Marquart <mads@marquart.dk>
- objc2-exception-helper 0.1.1: Mads Marquart <mads@marquart.dk>
- objc2-foundation 0.3.2: https://github.com/madsmtm/objc2
- objc2-quartz-core 0.3.2: https://github.com/madsmtm/objc2
- objc2-web-kit 0.3.2: https://github.com/madsmtm/objc2
- once_cell 1.21.4: Aleksey Kladov <aleksey.kladov@gmail.com>
- option-ext 0.2.0: (c) under Patent Claims infringed by Covered Software in the absence of
- parking_lot 0.12.5: Copyright (c) 2016 The Rust Project Developers
- parking_lot_core 0.9.12: Copyright (c) 2016 The Rust Project Developers
- percent-encoding 2.3.2: Copyright (c) 2013-2025 The rust-url developers
- phf 0.13.1: Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi
- phf_generator 0.13.1: Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi
- phf_macros 0.13.1: Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi
- phf_shared 0.13.1: Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi
- pin-project-lite 0.2.17: https://github.com/taiki-e/pin-project-lite
- plist 1.10.1: Copyright (c) 2015 Edward Barnard
- png 0.17.16: Copyright (c) 2015 nwin
- png 0.18.1: Copyright (c) 2015 nwin
- potential_utf 0.1.6: Copyright © 2020-2024 Unicode, Inc.
- powerfmt 0.2.1: Copyright 2023 Jacob Pratt et al. / Copyright (c) 2023 Jacob Pratt et al.
- precomputed-hash 0.1.1: Copyright (c) 2017 Emilio Cobos Álvarez
- proc-macro2 1.0.107: David Tolnay <dtolnay@gmail.com>, Alex Crichton <alex@alexcrichton.com>
- quick-xml 0.42.0: Copyright (c) 2016 Johann Tuffe
- quote 1.0.47: David Tolnay <dtolnay@gmail.com>
- raw-window-handle 0.6.2: Copyright (c) 2019 Osspial / Copyright (c) 2020 Osspial
- regex 1.13.1: Copyright (c) 2014 The Rust Project Developers
- regex-automata 0.4.18: Copyright (c) 2014 The Rust Project Developers
- regex-syntax 0.8.11: Copyright (c) 2014 The Rust Project Developers
- rustc-hash 2.1.3: The Rust Project Developers
- same-file 1.0.6: Copyright (c) 2017 Andrew Gallant
- schemars 0.8.22: Copyright (c) 2019 Graham Esau
- schemars_derive 0.8.22: Copyright (c) 2019 Graham Esau
- scopeguard 1.2.0: Copyright (c) 2016-2019 Ulrik Sverdrup "bluss" and scopeguard developers
- selectors 0.38.0: The Servo Project Developers
- semver 1.0.28: David Tolnay <dtolnay@gmail.com>
- serde 1.0.229: Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com>
- serde-untagged 0.1.9: David Tolnay <dtolnay@gmail.com>
- serde_core 1.0.229: Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com>
- serde_derive 1.0.229: Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com>
- serde_derive_internals 0.29.1: Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com>
- serde_json 1.0.151: Erick Tryzelaar <erick.tryzelaar@gmail.com>, David Tolnay <dtolnay@gmail.com>
- serde_repr 0.1.21: David Tolnay <dtolnay@gmail.com>
- serde_spanned 1.1.2: https://github.com/toml-rs/toml
- serde_with 3.24.0: Copyright (c) 2015
- serde_with_macros 3.24.0: Copyright (c) 2015
- serialize-to-javascript 0.1.2: Copyright (c) 2021 Chip Reed
- serialize-to-javascript-impl 0.1.2: Copyright (c) 2021 Chip Reed
- servo_arc 0.4.3: The Servo Project Developers
- sha2 0.10.9: Copyright (c) 2006-2009 Graydon Hoare / Copyright (c) 2009-2013 Mozilla Foundation / Copyright (c) 2016 Artyom Pavlov
- sha2 0.11.0: Copyright (c) 2016-2026 The RustCrypto Project Developers / Copyright (c) 2016 Artyom Pavlov / Copyright (c) 2009-2013 Mozilla Foundation / Copyright (c) 2006-2009 Graydon Hoare
- simd-adler32 0.3.10: Copyright (c) [2021] [Marvin Countryman]
- siphasher 1.0.4: Copyright 2012-2016 The Rust Project Developers. / Copyright 2016-2026 Frank Denis.
- smallvec 1.16.2: Copyright (c) 2018 The Servo Project Developers
- softbuffer 0.4.8: Copyright 2022 Kirill Chibisov
- sonora 0.2.0: Copyright (c) 2011, The WebRTC Project Authors. All rights reserved. / Copyright (c) 2016, Arun Raghavan and contributors. All rights reserved. / Copyright (c) 2026, dignifiedquire. All rights reserved.
- sonora-aec3 0.2.0: https://github.com/dignifiedquire/sonora
- sonora-agc2 0.2.0: Copyright (c) 2011, The WebRTC Project Authors. All rights reserved. / Copyright (c) 2016, Arun Raghavan and contributors. All rights reserved. / Copyright (c) 2026, dignifiedquire. All rights reserved.
- sonora-common-audio 0.2.0: Copyright (c) 2011, The WebRTC Project Authors. All rights reserved. / Copyright (c) 2016, Arun Raghavan and contributors. All rights reserved. / Copyright (c) 2026, dignifiedquire. All rights reserved.
- sonora-fft 0.2.0: Copyright (c) 2011, The WebRTC Project Authors. All rights reserved. / Copyright (c) 2016, Arun Raghavan and contributors. All rights reserved. / Copyright (c) 2026, dignifiedquire. All rights reserved.
- sonora-ns 0.2.0: Copyright (c) 2011, The WebRTC Project Authors. All rights reserved. / Copyright (c) 2016, Arun Raghavan and contributors. All rights reserved. / Copyright (c) 2026, dignifiedquire. All rights reserved.
- sonora-simd 0.2.0: Copyright (c) 2011, The WebRTC Project Authors. All rights reserved. / Copyright (c) 2016, Arun Raghavan and contributors. All rights reserved. / Copyright (c) 2026, dignifiedquire. All rights reserved.
- stable_deref_trait 1.2.1: Copyright (c) 2017 Robert Grosse
- string_cache 0.9.0: Copyright (c) 2012-2013 Mozilla Foundation
- strsim 0.11.1: Copyright (c) 2015 Danny Guo / Copyright (c) 2016 Titus Wormer <tituswormer@gmail.com> / Copyright (c) 2018 Akash Kurdekar
- swift-rs 1.0.8: Copyright 2023 The swift-rs developers / Copyright (c) 2023 The swift-rs Developers
- syn 2.0.119: David Tolnay <dtolnay@gmail.com>
- syn 3.0.6: David Tolnay <dtolnay@gmail.com>
- synstructure 0.14.0: Copyright 2016 Nika Layzell
- tao 0.37.1: Tauri Programme within The Commons Conservancy, The winit contributors
- tauri 2.12.2: Copyright (c) 2017 - Present Tauri Apps Contributors
- tauri-codegen 2.7.1: Copyright (c) 2017 - Present Tauri Apps Contributors
- tauri-macros 2.7.1: Copyright (c) 2017 - Present Tauri Apps Contributors
- tauri-runtime 2.12.1: Copyright (c) 2017 - Present Tauri Apps Contributors
- tauri-runtime-wry 2.12.1: Copyright (c) 2017 - Present Tauri Apps Contributors
- tauri-utils 2.10.1: Copyright (c) 2017 - Present Tauri Apps Contributors
- tendril 0.5.1: Copyright (c) 2015 Keegan McAllister
- thiserror 2.0.21: David Tolnay <dtolnay@gmail.com>
- thiserror-impl 2.0.21: David Tolnay <dtolnay@gmail.com>
- time 0.3.55: Jacob Pratt <open-source@jhpratt.dev>, Time contributors
- time-core 0.1.9: Jacob Pratt <open-source@jhpratt.dev>, Time contributors
- time-macros 0.2.32: Jacob Pratt <open-source@jhpratt.dev>, Time contributors
- tinystr 0.8.4: Copyright © 2020-2024 Unicode, Inc.
- tokio 1.53.2: Tokio Contributors <team@tokio.rs>
- toml 1.1.8+spec-1.1.0: https://github.com/toml-rs/toml
- toml_datetime 1.1.2+spec-1.1.0: https://github.com/toml-rs/toml
- toml_parser 1.1.5+spec-1.1.0: https://github.com/toml-rs/toml
- toml_writer 1.1.3+spec-1.1.0: https://github.com/toml-rs/toml
- tracing 0.1.44: Copyright (c) 2019 Tokio Contributors
- tracing-attributes 0.1.31: Copyright (c) 2019 Tokio Contributors
- tracing-core 0.1.36: Copyright (c) 2019 Tokio Contributors
- tray-icon 0.25.1: Copyright (c) 2022-2022 Tauri Programme within The Commons Conservancy
- typeid 1.0.3: David Tolnay <dtolnay@gmail.com>
- typenum 1.20.1: Copyright 2014 Paho Lurie-Gregg / Copyright (c) 2014 Paho Lurie-Gregg
- unicode-ident 1.0.26: Copyright © 1991-2023 Unicode, Inc.
- unicode-segmentation 1.13.3: Copyright (c) 2015 The Rust Project Developers
- unicode-xid 0.2.6: Copyright (c) 2015 The Rust Project Developers
- url 2.5.8: Copyright (c) 2013-2025 The rust-url developers
- urlpattern 0.6.0: Copyright (c) 2021 the Deno authors
- utf8_iter 1.0.4: Henri Sivonen <hsivonen@hsivonen.fi>
- uuid 1.27.0: Copyright (c) 2014 The Rust Project Developers / Copyright (c) 2018 Ashley Mannix, Christopher Armstrong, Dylan DPC, Hunar Roop Kahlon
- walkdir 2.5.0: Copyright (c) 2015 Andrew Gallant
- web-time 1.1.0: Copyright 2023 dAxpeDDa / Copyright (c) 2023 dAxpeDDa
- web_atoms 0.2.6: Copyright (c) 2014 The html5ever Project Developers
- webview2-com 0.39.1: https://github.com/wravery/webview2-rs
- webview2-com-macros 0.8.1: https://github.com/wravery/webview2-rs
- webview2-com-sys 0.39.1: https://github.com/wravery/webview2-rs
- winapi-util 0.1.11: Copyright (c) 2017 Andrew Gallant
- window-vibrancy 0.8.1: Copyright (c) 2020-2022 Tauri Programme within The Commons Conservancy
- windows 0.62.2: https://github.com/microsoft/windows-rs
- windows-collections 0.3.2: https://github.com/microsoft/windows-rs
- windows-core 0.62.2: https://github.com/microsoft/windows-rs
- windows-future 0.3.2: https://github.com/microsoft/windows-rs
- windows-implement 0.60.2: https://github.com/microsoft/windows-rs
- windows-interface 0.59.3: https://github.com/microsoft/windows-rs
- windows-link 0.2.1: https://github.com/microsoft/windows-rs
- windows-numerics 0.3.1: https://github.com/microsoft/windows-rs
- windows-result 0.4.1: https://github.com/microsoft/windows-rs
- windows-strings 0.5.1: https://github.com/microsoft/windows-rs
- windows-sys 0.60.2: Microsoft
- windows-sys 0.61.2: https://github.com/microsoft/windows-rs
- windows-targets 0.53.5: https://github.com/microsoft/windows-rs
- windows-threading 0.2.1: https://github.com/microsoft/windows-rs
- windows-version 0.1.7: https://github.com/microsoft/windows-rs
- windows_x86_64_msvc 0.53.1: https://github.com/microsoft/windows-rs
- winnow 1.0.4: https://github.com/winnow-rs/winnow
- writeable 0.6.4: Copyright © 2020-2024 Unicode, Inc.
- wry 0.57.0: Copyright (c) 2020-2023 Ngo Iok Ui & Tauri Programme within The Commons Conservancy
- yoke 0.8.3: Copyright © 2020-2024 Unicode, Inc.
- yoke-derive 0.8.4: Copyright © 2020-2024 Unicode, Inc.
- zerofrom 0.1.8: Copyright © 2020-2024 Unicode, Inc.
- zerofrom-derive 0.1.8: Copyright © 2020-2024 Unicode, Inc.
- zerotrie 0.2.5: Copyright © 2020-2024 Unicode, Inc.
- zerovec 0.11.8: Copyright © 2020-2024 Unicode, Inc.
- zerovec-derive 0.11.6: Copyright © 2020-2024 Unicode, Inc.
- zmij 1.0.23: David Tolnay <dtolnay@gmail.com>

## 3. Licence texts


### MIT License (whisper.cpp, llama.cpp, ONNX Runtime, local-stt and the MIT crates)

```text
whisper.cpp and llama.cpp: Copyright (c) 2023-2026 The ggml authors
ONNX Runtime: Copyright (c) Microsoft Corporation
local-stt: Copyright (c) 2026 local-stt contributors
Rust crates: the holders named in section 2.

Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

### BSD 3-Clause License (sonora, KISS FFT, brotli and related crates)

```text
sonora (all sonora-* crates):
Copyright (c) 2011, The WebRTC Project Authors. All rights reserved.
Copyright (c) 2016, Arun Raghavan and contributors. All rights reserved.
Copyright (c) 2026, dignifiedquire. All rights reserved.
KISS FFT: Copyright (c) 2003-2010 Mark Borgerding. All rights reserved.
Other crates: the holders named in section 2.

Copyright (c) 2011, The WebRTC Project Authors. All rights reserved.
Copyright (c) 2016, Arun Raghavan and contributors. All rights reserved.
Copyright (c) 2026, dignifiedquire. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

  * Redistributions of source code must retain the above copyright
    notice, this list of conditions and the following disclaimer.

  * Redistributions in binary form must reproduce the above copyright
    notice, this list of conditions and the following disclaimer in
    the documentation and/or other materials provided with the
    distribution.

  * Neither the name of Google nor the names of its contributors may
    be used to endorse or promote products derived from this software
    without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### Apache License 2.0 (sherpa-onnx and its bundled Apache-2.0 components, and the Apache-2.0 crates)

```text
                                 Apache License
                           Version 2.0, January 2004
                        http://www.apache.org/licenses/

   TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION

   1. Definitions.

      "License" shall mean the terms and conditions for use, reproduction,
      and distribution as defined by Sections 1 through 9 of this document.

      "Licensor" shall mean the copyright owner or entity authorized by
      the copyright owner that is granting the License.

      "Legal Entity" shall mean the union of the acting entity and all
      other entities that control, are controlled by, or are under common
      control with that entity. For the purposes of this definition,
      "control" means (i) the power, direct or indirect, to cause the
      direction or management of such entity, whether by contract or
      otherwise, or (ii) ownership of fifty percent (50%) or more of the
      outstanding shares, or (iii) beneficial ownership of such entity.

      "You" (or "Your") shall mean an individual or Legal Entity
      exercising permissions granted by this License.

      "Source" form shall mean the preferred form for making modifications,
      including but not limited to software source code, documentation
      source, and configuration files.

      "Object" form shall mean any form resulting from mechanical
      transformation or translation of a Source form, including but
      not limited to compiled object code, generated documentation,
      and conversions to other media types.

      "Work" shall mean the work of authorship, whether in Source or
      Object form, made available under the License, as indicated by a
      copyright notice that is included in or attached to the work
      (an example is provided in the Appendix below).

      "Derivative Works" shall mean any work, whether in Source or Object
      form, that is based on (or derived from) the Work and for which the
      editorial revisions, annotations, elaborations, or other modifications
      represent, as a whole, an original work of authorship. For the purposes
      of this License, Derivative Works shall not include works that remain
      separable from, or merely link (or bind by name) to the interfaces of,
      the Work and Derivative Works thereof.

      "Contribution" shall mean any work of authorship, including
      the original version of the Work and any modifications or additions
      to that Work or Derivative Works thereof, that is intentionally
      submitted to Licensor for inclusion in the Work by the copyright owner
      or by an individual or Legal Entity authorized to submit on behalf of
      the copyright owner. For the purposes of this definition, "submitted"
      means any form of electronic, verbal, or written communication sent
      to the Licensor or its representatives, including but not limited to
      communication on electronic mailing lists, source code control systems,
      and issue tracking systems that are managed by, or on behalf of, the
      Licensor for the purpose of discussing and improving the Work, but
      excluding communication that is conspicuously marked or otherwise
      designated in writing by the copyright owner as "Not a Contribution."

      "Contributor" shall mean Licensor and any individual or Legal Entity
      on behalf of whom a Contribution has been received by Licensor and
      subsequently incorporated within the Work.

   2. Grant of Copyright License. Subject to the terms and conditions of
      this License, each Contributor hereby grants to You a perpetual,
      worldwide, non-exclusive, no-charge, royalty-free, irrevocable
      copyright license to reproduce, prepare Derivative Works of,
      publicly display, publicly perform, sublicense, and distribute the
      Work and such Derivative Works in Source or Object form.

   3. Grant of Patent License. Subject to the terms and conditions of
      this License, each Contributor hereby grants to You a perpetual,
      worldwide, non-exclusive, no-charge, royalty-free, irrevocable
      (except as stated in this section) patent license to make, have made,
      use, offer to sell, sell, import, and otherwise transfer the Work,
      where such license applies only to those patent claims licensable
      by such Contributor that are necessarily infringed by their
      Contribution(s) alone or by combination of their Contribution(s)
      with the Work to which such Contribution(s) was submitted. If You
      institute patent litigation against any entity (including a
      cross-claim or counterclaim in a lawsuit) alleging that the Work
      or a Contribution incorporated within the Work constitutes direct
      or contributory patent infringement, then any patent licenses
      granted to You under this License for that Work shall terminate
      as of the date such litigation is filed.

   4. Redistribution. You may reproduce and distribute copies of the
      Work or Derivative Works thereof in any medium, with or without
      modifications, and in Source or Object form, provided that You
      meet the following conditions:

      (a) You must give any other recipients of the Work or
          Derivative Works a copy of this License; and

      (b) You must cause any modified files to carry prominent notices
          stating that You changed the files; and

      (c) You must retain, in the Source form of any Derivative Works
          that You distribute, all copyright, patent, trademark, and
          attribution notices from the Source form of the Work,
          excluding those notices that do not pertain to any part of
          the Derivative Works; and

      (d) If the Work includes a "NOTICE" text file as part of its
          distribution, then any Derivative Works that You distribute must
          include a readable copy of the attribution notices contained
          within such NOTICE file, excluding those notices that do not
          pertain to any part of the Derivative Works, in at least one
          of the following places: within a NOTICE text file distributed
          as part of the Derivative Works; within the Source form or
          documentation, if provided along with the Derivative Works; or,
          within a display generated by the Derivative Works, if and
          wherever such third-party notices normally appear. The contents
          of the NOTICE file are for informational purposes only and
          do not modify the License. You may add Your own attribution
          notices within Derivative Works that You distribute, alongside
          or as an addendum to the NOTICE text from the Work, provided
          that such additional attribution notices cannot be construed
          as modifying the License.

      You may add Your own copyright statement to Your modifications and
      may provide additional or different license terms and conditions
      for use, reproduction, or distribution of Your modifications, or
      for any such Derivative Works as a whole, provided Your use,
      reproduction, and distribution of the Work otherwise complies with
      the conditions stated in this License.

   5. Submission of Contributions. Unless You explicitly state otherwise,
      any Contribution intentionally submitted for inclusion in the Work
      by You to the Licensor shall be under the terms and conditions of
      this License, without any additional terms or conditions.
      Notwithstanding the above, nothing herein shall supersede or modify
      the terms of any separate license agreement you may have executed
      with Licensor regarding such Contributions.

   6. Trademarks. This License does not grant permission to use the trade
      names, trademarks, service marks, or product names of the Licensor,
      except as required for reasonable and customary use in describing the
      origin of the Work and reproducing the content of the NOTICE file.

   7. Disclaimer of Warranty. Unless required by applicable law or
      agreed to in writing, Licensor provides the Work (and each
      Contributor provides its Contributions) on an "AS IS" BASIS,
      WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or
      implied, including, without limitation, any warranties or conditions
      of TITLE, NON-INFRINGEMENT, MERCHANTABILITY, or FITNESS FOR A
      PARTICULAR PURPOSE. You are solely responsible for determining the
      appropriateness of using or redistributing the Work and assume any
      risks associated with Your exercise of permissions under this License.

   8. Limitation of Liability. In no event and under no legal theory,
      whether in tort (including negligence), contract, or otherwise,
      unless required by applicable law (such as deliberate and grossly
      negligent acts) or agreed to in writing, shall any Contributor be
      liable to You for damages, including any direct, indirect, special,
      incidental, or consequential damages of any character arising as a
      result of this License or out of the use or inability to use the
      Work (including but not limited to damages for loss of goodwill,
      work stoppage, computer failure or malfunction, or any and all
      other commercial damages or losses), even if such Contributor
      has been advised of the possibility of such damages.

   9. Accepting Warranty or Additional Liability. While redistributing
      the Work or Derivative Works thereof, You may choose to offer,
      and charge a fee for, acceptance of support, warranty, indemnity,
      or other liability obligations and/or rights consistent with this
      License. However, in accepting such obligations, You may act only
      on Your own behalf and on Your sole responsibility, not on behalf
      of any other Contributor, and only if You agree to indemnify,
      defend, and hold each Contributor harmless for any liability
      incurred by, or claims asserted against, such Contributor by reason
      of your accepting any such warranty or additional liability.

   END OF TERMS AND CONDITIONS

   APPENDIX: How to apply the Apache License to your work.

      To apply the Apache License to your work, attach the following
      boilerplate notice, with the fields enclosed by brackets "[]"
      replaced with your own identifying information. (Don't include
      the brackets!)  The text should be enclosed in the appropriate
      comment syntax for the file format. We also recommend that a
      file or class name and description of purpose be included on the
      same "printed page" as the copyright notice for easier
      identification within third-party archives.

   Copyright [yyyy] [name of copyright owner]

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.
```

No NOTICE file is shipped by sherpa-onnx's vendored licence directory or by any listed crate.


### Mozilla Public License 2.0 (cssparser, cssparser-macros, dtoa-short, option-ext, selectors)

```text
Mozilla Public License Version 2.0
==================================

1. Definitions
--------------

1.1. "Contributor"
    means each individual or legal entity that creates, contributes to
    the creation of, or owns Covered Software.

1.2. "Contributor Version"
    means the combination of the Contributions of others (if any) used
    by a Contributor and that particular Contributor's Contribution.

1.3. "Contribution"
    means Covered Software of a particular Contributor.

1.4. "Covered Software"
    means Source Code Form to which the initial Contributor has attached
    the notice in Exhibit A, the Executable Form of such Source Code
    Form, and Modifications of such Source Code Form, in each case
    including portions thereof.

1.5. "Incompatible With Secondary Licenses"
    means

    (a) that the initial Contributor has attached the notice described
        in Exhibit B to the Covered Software; or

    (b) that the Covered Software was made available under the terms of
        version 1.1 or earlier of the License, but not also under the
        terms of a Secondary License.

1.6. "Executable Form"
    means any form of the work other than Source Code Form.

1.7. "Larger Work"
    means a work that combines Covered Software with other material, in 
    a separate file or files, that is not Covered Software.

1.8. "License"
    means this document.

1.9. "Licensable"
    means having the right to grant, to the maximum extent possible,
    whether at the time of the initial grant or subsequently, any and
    all of the rights conveyed by this License.

1.10. "Modifications"
    means any of the following:

    (a) any file in Source Code Form that results from an addition to,
        deletion from, or modification of the contents of Covered
        Software; or

    (b) any new file in Source Code Form that contains any Covered
        Software.

1.11. "Patent Claims" of a Contributor
    means any patent claim(s), including without limitation, method,
    process, and apparatus claims, in any patent Licensable by such
    Contributor that would be infringed, but for the grant of the
    License, by the making, using, selling, offering for sale, having
    made, import, or transfer of either its Contributions or its
    Contributor Version.

1.12. "Secondary License"
    means either the GNU General Public License, Version 2.0, the GNU
    Lesser General Public License, Version 2.1, the GNU Affero General
    Public License, Version 3.0, or any later versions of those
    licenses.

1.13. "Source Code Form"
    means the form of the work preferred for making modifications.

1.14. "You" (or "Your")
    means an individual or a legal entity exercising rights under this
    License. For legal entities, "You" includes any entity that
    controls, is controlled by, or is under common control with You. For
    purposes of this definition, "control" means (a) the power, direct
    or indirect, to cause the direction or management of such entity,
    whether by contract or otherwise, or (b) ownership of more than
    fifty percent (50%) of the outstanding shares or beneficial
    ownership of such entity.

2. License Grants and Conditions
--------------------------------

2.1. Grants

Each Contributor hereby grants You a world-wide, royalty-free,
non-exclusive license:

(a) under intellectual property rights (other than patent or trademark)
    Licensable by such Contributor to use, reproduce, make available,
    modify, display, perform, distribute, and otherwise exploit its
    Contributions, either on an unmodified basis, with Modifications, or
    as part of a Larger Work; and

(b) under Patent Claims of such Contributor to make, use, sell, offer
    for sale, have made, import, and otherwise transfer either its
    Contributions or its Contributor Version.

2.2. Effective Date

The licenses granted in Section 2.1 with respect to any Contribution
become effective for each Contribution on the date the Contributor first
distributes such Contribution.

2.3. Limitations on Grant Scope

The licenses granted in this Section 2 are the only rights granted under
this License. No additional rights or licenses will be implied from the
distribution or licensing of Covered Software under this License.
Notwithstanding Section 2.1(b) above, no patent license is granted by a
Contributor:

(a) for any code that a Contributor has removed from Covered Software;
    or

(b) for infringements caused by: (i) Your and any other third party's
    modifications of Covered Software, or (ii) the combination of its
    Contributions with other software (except as part of its Contributor
    Version); or

(c) under Patent Claims infringed by Covered Software in the absence of
    its Contributions.

This License does not grant any rights in the trademarks, service marks,
or logos of any Contributor (except as may be necessary to comply with
the notice requirements in Section 3.4).

2.4. Subsequent Licenses

No Contributor makes additional grants as a result of Your choice to
distribute the Covered Software under a subsequent version of this
License (see Section 10.2) or under the terms of a Secondary License (if
permitted under the terms of Section 3.3).

2.5. Representation

Each Contributor represents that the Contributor believes its
Contributions are its original creation(s) or it has sufficient rights
to grant the rights to its Contributions conveyed by this License.

2.6. Fair Use

This License is not intended to limit any rights You have under
applicable copyright doctrines of fair use, fair dealing, or other
equivalents.

2.7. Conditions

Sections 3.1, 3.2, 3.3, and 3.4 are conditions of the licenses granted
in Section 2.1.

3. Responsibilities
-------------------

3.1. Distribution of Source Form

All distribution of Covered Software in Source Code Form, including any
Modifications that You create or to which You contribute, must be under
the terms of this License. You must inform recipients that the Source
Code Form of the Covered Software is governed by the terms of this
License, and how they can obtain a copy of this License. You may not
attempt to alter or restrict the recipients' rights in the Source Code
Form.

3.2. Distribution of Executable Form

If You distribute Covered Software in Executable Form then:

(a) such Covered Software must also be made available in Source Code
    Form, as described in Section 3.1, and You must inform recipients of
    the Executable Form how they can obtain a copy of such Source Code
    Form by reasonable means in a timely manner, at a charge no more
    than the cost of distribution to the recipient; and

(b) You may distribute such Executable Form under the terms of this
    License, or sublicense it under different terms, provided that the
    license for the Executable Form does not attempt to limit or alter
    the recipients' rights in the Source Code Form under this License.

3.3. Distribution of a Larger Work

You may create and distribute a Larger Work under terms of Your choice,
provided that You also comply with the requirements of this License for
the Covered Software. If the Larger Work is a combination of Covered
Software with a work governed by one or more Secondary Licenses, and the
Covered Software is not Incompatible With Secondary Licenses, this
License permits You to additionally distribute such Covered Software
under the terms of such Secondary License(s), so that the recipient of
the Larger Work may, at their option, further distribute the Covered
Software under the terms of either this License or such Secondary
License(s).

3.4. Notices

You may not remove or alter the substance of any license notices
(including copyright notices, patent notices, disclaimers of warranty,
or limitations of liability) contained within the Source Code Form of
the Covered Software, except that You may alter any license notices to
the extent required to remedy known factual inaccuracies.

3.5. Application of Additional Terms

You may choose to offer, and to charge a fee for, warranty, support,
indemnity or liability obligations to one or more recipients of Covered
Software. However, You may do so only on Your own behalf, and not on
behalf of any Contributor. You must make it absolutely clear that any
such warranty, support, indemnity, or liability obligation is offered by
You alone, and You hereby agree to indemnify every Contributor for any
liability incurred by such Contributor as a result of warranty, support,
indemnity or liability terms You offer. You may include additional
disclaimers of warranty and limitations of liability specific to any
jurisdiction.

4. Inability to Comply Due to Statute or Regulation
---------------------------------------------------

If it is impossible for You to comply with any of the terms of this
License with respect to some or all of the Covered Software due to
statute, judicial order, or regulation then You must: (a) comply with
the terms of this License to the maximum extent possible; and (b)
describe the limitations and the code they affect. Such description must
be placed in a text file included with all distributions of the Covered
Software under this License. Except to the extent prohibited by statute
or regulation, such description must be sufficiently detailed for a
recipient of ordinary skill to be able to understand it.

5. Termination
--------------

5.1. The rights granted under this License will terminate automatically
if You fail to comply with any of its terms. However, if You become
compliant, then the rights granted under this License from a particular
Contributor are reinstated (a) provisionally, unless and until such
Contributor explicitly and finally terminates Your grants, and (b) on an
ongoing basis, if such Contributor fails to notify You of the
non-compliance by some reasonable means prior to 60 days after You have
come back into compliance. Moreover, Your grants from a particular
Contributor are reinstated on an ongoing basis if such Contributor
notifies You of the non-compliance by some reasonable means, this is the
first time You have received notice of non-compliance with this License
from such Contributor, and You become compliant prior to 30 days after
Your receipt of the notice.

5.2. If You initiate litigation against any entity by asserting a patent
infringement claim (excluding declaratory judgment actions,
counter-claims, and cross-claims) alleging that a Contributor Version
directly or indirectly infringes any patent, then the rights granted to
You by any and all Contributors for the Covered Software under Section
2.1 of this License shall terminate.

5.3. In the event of termination under Sections 5.1 or 5.2 above, all
end user license agreements (excluding distributors and resellers) which
have been validly granted by You or Your distributors under this License
prior to termination shall survive termination.

************************************************************************
*                                                                      *
*  6. Disclaimer of Warranty                                           *
*  -------------------------                                           *
*                                                                      *
*  Covered Software is provided under this License on an "as is"       *
*  basis, without warranty of any kind, either expressed, implied, or  *
*  statutory, including, without limitation, warranties that the       *
*  Covered Software is free of defects, merchantable, fit for a        *
*  particular purpose or non-infringing. The entire risk as to the     *
*  quality and performance of the Covered Software is with You.        *
*  Should any Covered Software prove defective in any respect, You     *
*  (not any Contributor) assume the cost of any necessary servicing,   *
*  repair, or correction. This disclaimer of warranty constitutes an   *
*  essential part of this License. No use of any Covered Software is   *
*  authorized under this License except under this disclaimer.         *
*                                                                      *
************************************************************************

************************************************************************
*                                                                      *
*  7. Limitation of Liability                                          *
*  --------------------------                                          *
*                                                                      *
*  Under no circumstances and under no legal theory, whether tort      *
*  (including negligence), contract, or otherwise, shall any           *
*  Contributor, or anyone who distributes Covered Software as          *
*  permitted above, be liable to You for any direct, indirect,         *
*  special, incidental, or consequential damages of any character      *
*  including, without limitation, damages for lost profits, loss of    *
*  goodwill, work stoppage, computer failure or malfunction, or any    *
*  and all other commercial damages or losses, even if such party      *
*  shall have been informed of the possibility of such damages. This   *
*  limitation of liability shall not apply to liability for death or   *
*  personal injury resulting from such party's negligence to the       *
*  extent applicable law prohibits such limitation. Some               *
*  jurisdictions do not allow the exclusion or limitation of           *
*  incidental or consequential damages, so this exclusion and          *
*  limitation may not apply to You.                                    *
*                                                                      *
************************************************************************

8. Litigation
-------------

Any litigation relating to this License may be brought only in the
courts of a jurisdiction where the defendant maintains its principal
place of business and such litigation shall be governed by laws of that
jurisdiction, without reference to its conflict-of-law provisions.
Nothing in this Section shall prevent a party's ability to bring
cross-claims or counter-claims.

9. Miscellaneous
----------------

This License represents the complete agreement concerning the subject
matter hereof. If any provision of this License is held to be
unenforceable, such provision shall be reformed only to the extent
necessary to make it enforceable. Any law or regulation which provides
that the language of a contract shall be construed against the drafter
shall not be used to construe this License against a Contributor.

10. Versions of the License
---------------------------

10.1. New Versions

Mozilla Foundation is the license steward. Except as provided in Section
10.3, no one other than the license steward has the right to modify or
publish new versions of this License. Each version will be given a
distinguishing version number.

10.2. Effect of New Versions

You may distribute the Covered Software under the terms of the version
of the License under which You originally received the Covered Software,
or under the terms of any subsequent version published by the license
steward.

10.3. Modified Versions

If you create software not governed by this License, and you want to
create a new license for such software, you may create and use a
modified version of this License if you rename the license and remove
any references to the name of the license steward (except to note that
such modified license differs from this License).

10.4. Distributing Source Code Form that is Incompatible With Secondary
Licenses

If You choose to distribute Source Code Form that is Incompatible With
Secondary Licenses under the terms of this version of the License, the
notice described in Exhibit B of this License must be attached.

Exhibit A - Source Code Form License Notice
-------------------------------------------

  This Source Code Form is subject to the terms of the Mozilla Public
  License, v. 2.0. If a copy of the MPL was not distributed with this
  file, You can obtain one at http://mozilla.org/MPL/2.0/.

If it is not possible or desirable to put the notice in a particular
file, then You may include the notice in a location (such as a LICENSE
file in a relevant directory) where a recipient would be likely to look
for such a notice.

You may add additional accurate notices of copyright ownership.

Exhibit B - "Incompatible With Secondary Licenses" Notice
---------------------------------------------------------

  This Source Code Form is "Incompatible With Secondary Licenses", as
  defined by the Mozilla Public License, v. 2.0.
```

The source of these crates is unmodified and available from https://crates.io under the names and versions listed in section 2.


### Boost Software License 1.0 (clipboard-win, error-code)

```text
Boost Software License - Version 1.0 - August 17th, 2003

Permission is hereby granted, free of charge, to any person or organization
obtaining a copy of the software and accompanying documentation covered by
this license (the "Software") to use, reproduce, display, distribute,
execute, and transmit the Software, and to prepare derivative works of the
Software, and to permit third-parties to whom the Software is furnished to
do so, all subject to the following:

The copyright notices in the Software and this entire statement, including
the above license grant, this restriction and the following disclaimer,
must be included in all copies of the Software, in whole or in part, and
all derivative works of the Software, unless such copies or derivative
works are solely in the form of machine-executable object code generated by
a source language processor.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE, TITLE AND NON-INFRINGEMENT. IN NO EVENT
SHALL THE COPYRIGHT HOLDERS OR ANYONE DISTRIBUTING THE SOFTWARE BE LIABLE
FOR ANY DAMAGES OR OTHER LIABILITY, WHETHER IN CONTRACT, TORT OR OTHERWISE,
ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

### zlib License (foldhash)

```text
Copyright (c) 2024 Orson Peters

This software is provided 'as-is', without any express or implied warranty. In
no event will the authors be held liable for any damages arising from the use of
this software.

Permission is granted to anyone to use this software for any purpose, including
commercial applications, and to alter it and redistribute it freely, subject to
the following restrictions:

1. The origin of this software must not be misrepresented; you must not claim
    that you wrote the original software. If you use this software in a product,
    an acknowledgment in the product documentation would be appreciated but is
    not required.

2. Altered source versions must be plainly marked as such, and must not be
    misrepresented as being the original software.

3. This notice may not be removed or altered from any source distribution.
```

### Unicode License v3 (icu_* and related crates, unicode-ident)

```text
UNICODE LICENSE V3

COPYRIGHT AND PERMISSION NOTICE

Copyright © 1991-2023 Unicode, Inc.

NOTICE TO USER: Carefully read the following legal agreement. BY
DOWNLOADING, INSTALLING, COPYING OR OTHERWISE USING DATA FILES, AND/OR
SOFTWARE, YOU UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE
TERMS AND CONDITIONS OF THIS AGREEMENT. IF YOU DO NOT AGREE, DO NOT
DOWNLOAD, INSTALL, COPY, DISTRIBUTE OR USE THE DATA FILES OR SOFTWARE.

Permission is hereby granted, free of charge, to any person obtaining a
copy of data files and any associated documentation (the "Data Files") or
software and any associated documentation (the "Software") to deal in the
Data Files or Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, and/or sell
copies of the Data Files or Software, and to permit persons to whom the
Data Files or Software are furnished to do so, provided that either (a)
this copyright and permission notice appear with all copies of the Data
Files or Software, or (b) this copyright and permission notice appear in
associated Documentation.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF
THIRD PARTY RIGHTS.

IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS NOTICE
BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL DAMAGES,
OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THE DATA
FILES OR SOFTWARE.

Except as contained in this notice, the name of a copyright holder shall
not be used in advertising or otherwise to promote the sale, use or other
dealings in these Data Files or Software without prior written
authorization of the copyright holder.
```

## 4. Models downloaded at run time

Not redistributed with the wheel. Each model stays under its own licence, shown on its download page.

| Model | Licence | Where |
|---|---|---|
| Whisper small, medium and large-v3 (q5 GGML files) | MIT (OpenAI; GGML conversion by ggml-org) | https://huggingface.co/ggerganov/whisper.cpp |
| IBM Granite 4.0 Micro (Q4_K_M GGUF) | Apache-2.0 | https://huggingface.co/ibm-granite/granite-4.0-micro-GGUF |
| Google Gemma 4 E4B instruction-tuned (Q4_0 GGUF) | Apache-2.0 per its model card; Google's Gemma terms are linked there | https://huggingface.co/google/gemma-4-E4B-it-qat-q4_0-gguf |
| 3D-Speaker CAM++ speaker model (ONNX, via sherpa-onnx releases) | Apache-2.0 (3D-Speaker project, ModelScope) | https://github.com/modelscope/3D-Speaker |

