use std::env;
use std::path::PathBuf;

fn main() {
    let qwen_c_dir = PathBuf::from("qwen3-tts-c");
    
    // Tell cargo to rerun if any C files change
    println!("cargo:rerun-if-changed=qwen3-tts-c/qwen_tts.c");
    println!("cargo:rerun-if-changed=qwen3-tts-c/qwen_tts.h");
    println!("cargo:rerun-if-changed=qwen3-tts-c/qwen_tts_server.c");
    println!("cargo:rerun-if-changed=qwen3-tts-c/qwen_tts_server.h");
    println!("cargo:rerun-if-changed=qwen3-tts-c/qwen_tts_thread.c");
    println!("cargo:rerun-if-changed=qwen3-tts-c/qwen_tts_thread.h");

    // Determine target OS and set appropriate flags
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    
    // Core library files (excluding benchmarks, tests, and main.c)
    let mut c_files = vec![
        "qwen_tts.c",
        "qwen_tts_server.c",
        "qwen_tts_thread.c",
        "qwen_tts_audio.c",
        "qwen_tts_backend.c",
        "qwen_tts_code_predictor.c",
        "qwen_tts_compose.c",
        "qwen_tts_emotion.c",
        "qwen_tts_kernels.c",
        "qwen_tts_kernels_generic.c",
        "qwen_tts_safetensors.c",
        "qwen_tts_sampling.c",
        "qwen_tts_speech_decoder.c",
        "qwen_tts_speech_encoder.c",
        "qwen_tts_talker.c",
        "qwen_tts_tokenizer.c",
        "qwen_tts_voice_clone.c",
    ];

    // Add platform-specific kernel implementations
    // Linux/Android uses NEON or AVX, macOS has its own impl
    if target_os == "linux" || target_os == "android" {
        c_files.push("qwen_tts_kernels_neon.c");
    }
    // Note: qwen_tts_kernels_avx.c for x86_64 Linux could be added conditionally
    
    let mut build = cc::Build::new();
    build
        .files(c_files.iter().map(|f| qwen_c_dir.join(f).to_str().unwrap()))
        .include(&qwen_c_dir)
        .flag_if_supported("-O2")
        .flag_if_supported("-ffast-math")
        .define("_GNU_SOURCE", None);

    // Platform-specific flags
    if target_os == "macos" {
        // GCD is used on macOS for parallelism
        build.flag("-DUSE_GCD");
    } else if target_os == "linux" {
        // pthread for Linux
        build.flag("-DUSE_PTHREAD");
    }

    build.compile("qwen_tts_c");
    
    // Link pthread on Unix-like systems
    if target_os != "windows" {
        println!("cargo:rustc-link-lib=pthread");
    }
}
