fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let mut cfg = cmake::Config::new("vendor/whisper.cpp");
    cfg.define("BUILD_SHARED_LIBS", "OFF")
        .define("WHISPER_BUILD_EXAMPLES", "OFF")
        .define("WHISPER_BUILD_TESTS", "OFF")
        .define("WHISPER_BUILD_SERVER", "OFF")
        .define("GGML_NATIVE", "OFF")
        .profile("Release");
    if target_os == "macos" {
        cfg.define("GGML_METAL", "ON")
            .define("GGML_METAL_EMBED_LIBRARY", "ON")
            .define("GGML_BLAS", "ON");
    }
    let dst = cfg.build();

    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .file("shim.cpp")
        .include("vendor/whisper.cpp/include")
        .include("vendor/whisper.cpp/ggml/include")
        .compile("lstt_shim");

    link_llama(&dst);

    for sub in ["lib", "lib64"] {
        println!("cargo:rustc-link-search=native={}/{}", dst.display(), sub);
    }
    for lib in ["whisper", "ggml", "ggml-base", "ggml-cpu"] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    if target_os == "macos" {
        for lib in ["ggml-metal", "ggml-blas"] {
            println!("cargo:rustc-link-lib=static={lib}");
        }
        for fw in [
            "Foundation",
            "Metal",
            "MetalKit",
            "Accelerate",
            "CoreGraphics",
            "ApplicationServices",
            "CoreFoundation",
        ] {
            println!("cargo:rustc-link-lib=framework={fw}");
        }
        println!("cargo:rustc-link-lib=c++");
        // Info.plist inside the binary gives the mic prompt its text outside a .app bundle.
        let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        println!(
            "cargo:rustc-link-arg-bins=-Wl,-sectcreate,__TEXT,__info_plist,{manifest}/Info.plist"
        );
        // ggml-metal uses @available checks, which need compiler-rt when the deployment target is older than the SDK.
        if let Some(dir) = clang_runtime_dir() {
            println!("cargo:rustc-link-search=native={dir}");
            println!("cargo:rustc-link-lib=static=clang_rt.osx");
        }
    }
    println!("cargo:rerun-if-changed=shim.cpp");
    println!("cargo:rerun-if-changed=Info.plist");
    println!("cargo::rustc-check-cfg=cfg(lstt_voices)");
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_os == "macos" && target_arch == "aarch64" {
        link_voices();
    }
    // Listing the commands makes Tauri require a capability for each, so every window gets only what it needs.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "get_state",
        "set_config",
        "start_hotkey_capture",
        "cancel_hotkey_capture",
        "choose_model",
        "open_model_page",
        "delete_model",
        "choose_summary_model",
        "open_summary_model_page",
        "delete_summary_model",
        "summarize_note",
        "get_history",
        "clear_history",
        "delete_history",
        "get_insights",
        "reset_insights",
        "check_microphone",
        "export_diagnostics",
        "list_notes",
        "get_note",
        "start_note",
        "stop_note",
        "rename_note",
        "rename_speaker",
        "delete_notes",
        "open_accessibility_settings",
        "reinstall_app",
        "reset_overlay_position",
        "move_overlay",
        "copy_text",
        "install_app",
        "check_for_updates",
        "install_update",
        "skip_update",
        "open_release_notes",
        "take_update_notice",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("tauri build step failed");
}

// Note summaries: llama.cpp's libllama alone, built on whisper.cpp's ggml so one copy of ggml is linked.
fn link_llama(whisper_dst: &std::path::Path) {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("llama");
    let ggml_dir = whisper_dst.join("lib").join("cmake").join("ggml");
    let dst = cmake::Config::new("vendor/llama.cpp")
        .out_dir(&out)
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("LLAMA_USE_SYSTEM_GGML", "ON")
        .define("ggml_DIR", &ggml_dir)
        .define("LLAMA_BUILD_COMMON", "OFF")
        .define("LLAMA_BUILD_TESTS", "OFF")
        .define("LLAMA_BUILD_TOOLS", "OFF")
        .define("LLAMA_BUILD_EXAMPLES", "OFF")
        .define("LLAMA_BUILD_SERVER", "OFF")
        .define("LLAMA_BUILD_APP", "OFF")
        .define("LLAMA_CURL", "OFF")
        .profile("Release")
        .build();
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .file("llm_shim.cpp")
        .include("vendor/llama.cpp/include")
        .include("vendor/whisper.cpp/ggml/include")
        .compile("lstt_llm_shim");
    for sub in ["lib", "lib64"] {
        println!("cargo:rustc-link-search=native={}/{}", dst.display(), sub);
    }
    println!("cargo:rustc-link-lib=static=llama");
    println!("cargo:rerun-if-changed=llm_shim.cpp");
}

// Telling speakers apart in notes: sherpa-onnx's prebuilt static library (Apache-2.0), pinned by checksum.
const SHERPA_URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/v1.13.8/sherpa-onnx-v1.13.8-osx-arm64-static-no-tts-lib.tar.bz2";
const SHERPA_SHA256: &str = "3d7f9b8a496694af13d9802c33b8133231e397bdef302f543d19468765e83136";
const SHERPA_DIR: &str = "sherpa-onnx-v1.13.8-osx-arm64-static-no-tts-lib";
const SHERPA_LIBS: [&str; 10] = [
    "sherpa-onnx-c-api",
    "sherpa-onnx-core",
    "kaldi-native-fbank-core",
    "kissfft-float",
    "sherpa-onnx-kaldifst-core",
    "sherpa-onnx-fst",
    "sherpa-onnx-fstfar",
    "kaldi-decoder-core",
    "ssentencepiece_core",
    "onnxruntime",
];

fn link_voices() {
    let lib = sherpa_lib_dir();
    cc::Build::new()
        .file("voice_shim.c")
        .include("vendor/sherpa-onnx")
        .compile("lstt_voice_shim");
    println!("cargo:rustc-link-search=native={}", lib.display());
    for name in SHERPA_LIBS {
        println!("cargo:rustc-link-lib=static={name}");
    }
    println!("cargo:rustc-cfg=lstt_voices");
    println!("cargo:rerun-if-changed=voice_shim.c");
    println!("cargo:rerun-if-env-changed=LOCAL_STT_SHERPA_LIB");
}

// LOCAL_STT_SHERPA_LIB points at an unpacked lib folder for offline builds; otherwise it is fetched once per target dir.
fn sherpa_lib_dir() -> std::path::PathBuf {
    if let Some(dir) = std::env::var_os("LOCAL_STT_SHERPA_LIB") {
        return dir.into();
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let lib = out.join(SHERPA_DIR).join("lib");
    if lib.join("libsherpa-onnx-c-api.a").exists() {
        return lib;
    }
    let archive = out.join("sherpa-onnx.tar.bz2");
    run(std::process::Command::new("curl")
        .args(["-fsSL", "--retry", "3", "-o"])
        .arg(&archive)
        .arg(SHERPA_URL));
    let sum = std::process::Command::new("shasum")
        .args(["-a", "256"])
        .arg(&archive)
        .output()
        .expect("shasum");
    let sum = String::from_utf8_lossy(&sum.stdout);
    assert_eq!(
        sum.split_whitespace().next(),
        Some(SHERPA_SHA256),
        "sherpa-onnx archive checksum mismatch"
    );
    run(std::process::Command::new("tar")
        .arg("-xjf")
        .arg(&archive)
        .arg("-C")
        .arg(&out));
    let _ = std::fs::remove_file(&archive);
    lib
}

fn run(cmd: &mut std::process::Command) {
    let status = cmd.status().expect("cannot start a build tool");
    assert!(status.success(), "{cmd:?} failed");
}

fn clang_runtime_dir() -> Option<String> {
    let out = std::process::Command::new("xcrun")
        .args(["clang", "--print-runtime-dir"])
        .output()
        .ok()?;
    let dir = String::from_utf8(out.stdout).ok()?.trim().to_string();
    std::path::Path::new(&dir)
        .join("libclang_rt.osx.a")
        .exists()
        .then_some(dir)
}
