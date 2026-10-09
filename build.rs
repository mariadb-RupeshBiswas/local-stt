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
    // Listing the commands makes Tauri require a capability for each, so every window gets only what it needs.
    let manifest = tauri_build::AppManifest::new().commands(&[
        "get_state",
        "set_config",
        "start_hotkey_capture",
        "cancel_hotkey_capture",
        "choose_model",
        "get_history",
        "clear_history",
        "reset_overlay_position",
        "move_overlay",
        "copy_text",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("tauri build step failed");
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
