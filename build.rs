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
        .file("shim.c")
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
    }
    println!("cargo:rerun-if-changed=shim.c");
    println!("cargo:rerun-if-changed=Info.plist");
    tauri_build::build();
}
