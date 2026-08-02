use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let c_dir = manifest_dir.join("qwen3-tts-c");
    let glue_dir = manifest_dir.join("glue");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    let mut build = cc::Build::new();

    build
        .include(&c_dir)
        .include(c_dir.join("vendor"))
        .include(&glue_dir)
        .flag("-Wall")
        .flag("-Wextra")
        .flag("-O3")
        .define("USE_BLAS", None);

    // Architecture-specific flags
    if target_os == "darwin" {
        build.flag("-march=native");
        build.flag("-ffast-math");
        build.define("ACCELERATE_NEW_LAPACK", None);
    } else if target_arch == "x86_64" || target_arch == "x86" {
        // Linux/Windows x86_64: use AVX2+FMA by default (Haswell 2013+)
        if target_env == "msvc" {
            build.flag("/arch:AVX2");
        } else {
            build.flag("-mavx2");
            build.flag("-mfma");
            build.flag("-ffast-math");
        }
    } else if target_arch == "aarch64" {
        build.flag("-march=native");
        build.flag("-ffast-math");
    }

    build
        .file(c_dir.join("qwen_tts.c"))
        .file(c_dir.join("qwen_tts_talker.c"))
        .file(c_dir.join("qwen_tts_code_predictor.c"))
        .file(c_dir.join("qwen_tts_speech_decoder.c"))
        .file(c_dir.join("qwen_tts_kernels.c"))
        .file(c_dir.join("qwen_tts_thread.c"))
        .file(c_dir.join("qwen_tts_kernels_generic.c"))
        .file(c_dir.join("qwen_tts_kernels_neon.c"))
        .file(c_dir.join("qwen_tts_kernels_avx.c"))
        .file(c_dir.join("qwen_tts_audio.c"))
        .file(c_dir.join("qwen_tts_emotion.c"))
        .file(c_dir.join("qwen_tts_compose.c"))
        .file(c_dir.join("qwen_tts_sampling.c"))
        .file(c_dir.join("qwen_tts_tokenizer.c"))
        .file(c_dir.join("qwen_tts_safetensors.c"))
        .file(c_dir.join("qwen_tts_voice_clone.c"))
        .file(c_dir.join("vendor").join("lz4.c"))
        .file(glue_dir.join("ctx_accessors.c"));

    build.compile("qwen_tts");

    let mut speech_enc_build = cc::Build::new();
    speech_enc_build
        .include(&c_dir)
        .include(c_dir.join("vendor"))
        .flag("-Wall")
        .flag("-Wextra")
        .flag("-O3")
        .define("USE_BLAS", None);

    if target_os == "darwin" {
        speech_enc_build.flag("-march=native");
        speech_enc_build.define("ACCELERATE_NEW_LAPACK", None);
    } else if target_arch == "x86_64" || target_arch == "x86" {
        if target_env == "msvc" {
            speech_enc_build.flag("/arch:AVX2");
        } else {
            speech_enc_build.flag("-mavx2");
            speech_enc_build.flag("-mfma");
        }
    } else if target_arch == "aarch64" {
        speech_enc_build.flag("-march=native");
    }

    speech_enc_build
        .file(c_dir.join("qwen_tts_speech_encoder.c"))
        .compile("qwen_tts_speech_encoder");

    // Link libraries based on platform
    if target_os == "darwin" {
        println!("cargo:rustc-link-lib=framework=Accelerate");
        println!("cargo:rustc-link-lib=framework=Foundation");
    } else if target_os == "linux" {
        println!("cargo:rustc-link-lib=openblas");
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=pthread");
        // OpenBLAS include path may vary by distro
        if PathBuf::from("/usr/include/openblas").exists() {
            println!("cargo:rustc-link-search=native=/usr/lib/openblas");
        }
    } else if target_os == "windows" {
        // Windows: link openblas.lib (user must provide via vcpkg or manual install)
        println!("cargo:rustc-link-lib=openblas");
        println!("cargo:rustc-link-lib=ws2_32");  // for networking if needed
    }

    let bindings = bindgen::Builder::default()
        .clang_arg(format!("-I{}", c_dir.display()))
        .clang_arg(format!("-I{}", c_dir.join("vendor").display()))
        .clang_arg(format!("-I{}", glue_dir.display()))
        .header(
            c_dir
                .join("qwen_tts.h")
                .to_str()
                .unwrap(),
        )
        .header(
            c_dir
                .join("qwen_tts_emotion.h")
                .to_str()
                .unwrap(),
        )
        .header(
            c_dir
                .join("qwen_tts_compose.h")
                .to_str()
                .unwrap(),
        )
        .header(
            c_dir
                .join("qwen_tts_audio.h")
                .to_str()
                .unwrap(),
        )
        .header(
            c_dir
                .join("qwen_tts_thread.h")
                .to_str()
                .unwrap(),
        )
        .header(
            c_dir
                .join("qwen_tts_kernels.h")
                .to_str()
                .unwrap(),
        )
        .header(
            c_dir
                .join("qwen_tts_voice_clone.h")
                .to_str()
                .unwrap(),
        )
        .header(glue_dir.join("ctx_accessors.h").to_str().unwrap())
        .allowlist_function("qwen_.*")
        .allowlist_function("qwen_tts_.*")
        .allowlist_function("qwen_audio_.*")
        .allowlist_function("qwen_compose_.*")
        .allowlist_function("qwen_speech_encoder_.*")
        .allowlist_function("qwen_speaker_encoder_.*")
        .allowlist_function("qwen_extract_speaker_embedding")
        .allowlist_function("qwen_read_wav")
        .allowlist_function("qwen_trim_trailing_silence")
        .allowlist_function("qwen_mel_spectrogram")
        .allowlist_function("qwen_track_override")
        .allowlist_function("qwen_ctx_.*")
        .allowlist_var("QWEN_TTS_.*")
        .allowlist_var("QWEN_TTS_MAX_.*")
        .allowlist_var("Q4_0_BLOCK_SIZE")
        .layout_tests(false)
        .generate_comments(false)
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings");
}
