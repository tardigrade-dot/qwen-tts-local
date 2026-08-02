#[allow(dead_code)]
mod bindings;
mod audio;
mod ffi;
mod server;

use crate::bindings::*;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Parser)]
#[command(name = "qwen-tts", about = "Qwen3-TTS Rust front-end")]
struct Cli {
    #[arg(short = 'd', long = "model-dir", help = "Model directory")]
    model_dir: Option<String>,

    #[arg(short = 't', long = "text", help = "Text to synthesize")]
    text: Option<String>,

    #[arg(short = 'o', long = "output", default_value = "output.wav", help = "Output WAV file")]
    output: PathBuf,

    #[arg(short = 's', long = "speaker", default_value = "ryan", help = "Speaker name")]
    speaker: String,

    #[arg(short = 'l', long = "language", default_value = "English", help = "Language name")]
    language: String,

    #[arg(short = 'T', long = "temperature", default_value_t = 0.5, help = "Sampling temperature")]
    temperature: f32,

    #[arg(long = "top-k", default_value_t = 50, help = "Top-k sampling")]
    top_k: i32,

    #[arg(long = "top-p", default_value_t = 1.0, help = "Top-p sampling")]
    top_p: f32,

    #[arg(long = "rep-penalty", default_value_t = 1.05, help = "Repetition penalty")]
    rep_penalty: f32,

    #[arg(long = "seed", default_value_t = 0, help = "Random seed (0 = time-based)")]
    seed: u32,

    #[arg(long = "max-tokens", default_value_t = 8192, help = "Max tokens")]
    max_tokens: i32,

    #[arg(short = 'j', long = "threads", default_value_t = 0, help = "Threads (0 = auto)")]
    threads: i32,

    #[arg(long = "int8", help = "INT8 quantization")]
    int8: bool,

    #[arg(long = "int4", help = "INT4 quantization")]
    int4: bool,

    #[arg(long = "silent", help = "Suppress output")]
    silent: bool,

    #[arg(long = "stream", help = "Streaming synthesis")]
    stream: bool,

    #[arg(long = "stdout", help = "Write raw s16le PCM to stdout")]
    stdout: bool,

    #[arg(long = "stream-chunk", default_value_t = 10, help = "Frames per stream chunk")]
    stream_chunk: i32,

    #[arg(long = "instruct", help = "Style instruction (1.7B)")]
    instruct: Option<String>,

    #[arg(long = "emotion", help = "Emotion name")]
    emotion: Option<String>,

    #[arg(long = "volume", help = "Volume multiplier")]
    volume: Option<f32>,

    #[arg(long = "rate", help = "Playback rate")]
    rate: Option<f32>,

    #[arg(long = "roughness", help = "Text roughness")]
    roughness: Option<f32>,

    #[arg(long = "no-compose", help = "Disable compose/markup auto-detect")]
    no_compose: bool,

    #[arg(long = "ref-audio", help = "Reference audio for voice cloning")]
    ref_audio: Option<String>,

    #[arg(long = "ref-text", help = "Reference text for ICL")]
    ref_text: Option<String>,

    #[arg(long = "voice-design", help = "VoiceDesign mode (1.7B)")]
    voice_design: bool,

    #[arg(long = "xvector-only", help = "X-vector only voice clone")]
    xvector_only: bool,

    #[arg(long = "serve", help = "Start HTTP server on PORT")]
    serve: Option<u16>,

    #[arg(long = "workers", default_value_t = 1, help = "HTTP server workers")]
    workers: i32,

    #[arg(long = "idle-secs", help = "Idle seconds before unloading model")]
    idle_secs: Option<u64>,

    #[arg(long = "max-duration", help = "Max audio duration in seconds")]
    max_duration: Option<f32>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Serve {
        #[arg(short = 'd', long = "model-dir")]
        model_dir: String,

        #[arg(short = 'p', long = "port", default_value_t = 8080)]
        port: u16,

        #[arg(long = "int8")]
        int8: bool,

        #[arg(long = "int4")]
        int4: bool,

        #[arg(long = "silent")]
        silent: bool,

        #[arg(long = "ref-audio")]
        ref_audio: Option<String>,

        #[arg(long = "ref-text")]
        ref_text: Option<String>,

        #[arg(long = "workers", default_value_t = 1)]
        workers: i32,

        #[arg(long = "idle-secs")]
        idle_secs: Option<u64>,
    },
}

fn main() {
    let cli = Cli::parse();

    unsafe {
        qwen_check_runtime_isa();
    }

    if cli.threads > 0 {
        unsafe {
            qwen_set_threads(cli.threads);
        }
    } else {
        unsafe {
            qwen_init_threads();
        }
    }

    if let Some(Commands::Serve {
        model_dir,
        port,
        int8,
        int4,
        silent,
        ref_audio,
        ref_text,
        workers: _,
        idle_secs,
    }) = cli.command
    {
        run_server(
            &model_dir,
            port,
            silent,
            int8,
            int4,
            ref_audio,
            ref_text,
            idle_secs,
        );
        return;
    }

    if let Some(port) = cli.serve {
        let model_dir = cli.model_dir.clone().unwrap_or_else(|| {
            eprintln!("Error: --model-dir/-d is required");
            process::exit(1);
        });
        run_server(
            &model_dir,
            port,
            cli.silent,
            cli.int8,
            cli.int4,
            cli.ref_audio,
            cli.ref_text,
            cli.idle_secs,
        );
        return;
    }

    match run_cli(cli) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    }
}

fn run_cli(cli: Cli) -> Result<()> {
    let model_dir = cli.model_dir.as_deref().unwrap_or_else(|| {
        eprintln!("Error: --model-dir/-d is required");
        process::exit(1);
    });

    let text = cli.text.as_deref().unwrap_or_else(|| {
        eprintln!("Error: --text/-t is required");
        process::exit(1);
    });

    let ctx = unsafe {
        ffi::Context::load(model_dir, cli.silent, cli.int8, cli.int4)
            .ok_or_else(|| anyhow::anyhow!("Failed to load model"))?
    };
    let ctx_ptr = ctx.as_ptr();

    let params = ffi::GenerationParams {
        text: text.to_string(),
        speaker_id: unsafe { ffi::speaker_id(&cli.speaker) },
        language_id: unsafe { ffi::language_id(&cli.language) },
        temperature: cli.temperature,
        top_k: cli.top_k,
        top_p: cli.top_p,
        rep_penalty: cli.rep_penalty,
        seed: cli.seed,
        max_tokens: cli.max_tokens,
        instruct: cli.instruct.clone(),
        voice_design: cli.voice_design,
        voice_clone: cli.ref_audio.is_some(),
        xvector_only: cli.xvector_only,
        ref_audio_path: cli.ref_audio.clone(),
        ref_text: cli.ref_text.clone(),
        cp_roughness: cli.roughness.unwrap_or(0.0),
        emotion: cli.emotion.clone(),
        volume: cli.volume,
        rate: cli.rate,
    };

    if params.speaker_id < 0 {
        anyhow::bail!("Unknown speaker: {}", cli.speaker);
    }
    if params.language_id < 0 {
        anyhow::bail!("Unknown language: {}", cli.language);
    }

    unsafe {
        ffi::reset_context_state(ctx_ptr);
        ffi::apply_params_to_context(ctx_ptr, &params);
    }

    if let Some(ref emotion) = params.emotion {
        unsafe {
            ffi::apply_emotion(ctx_ptr, emotion, &cli.language);
        }
    }

    let use_compose = !cli.no_compose && has_markup(text);

    if cli.stream || cli.stdout {
        synthesize_stream(
            ctx_ptr,
            &params,
            &cli.output,
            cli.stdout,
            cli.stream_chunk,
            use_compose,
        )?;
    } else if use_compose {
        let pcm = generate_compose_full(ctx_ptr, &params)?;
        let mut result = pcm;
        apply_dsp(&mut result, cli.volume, cli.rate);
        let wav = audio::build_wav_in_memory(&result);
        fs::write(&cli.output, wav)?;
        if !cli.silent {
            let dur = result.len() as f32 / 24000.0;
            eprintln!("Wrote {} ({dur:.1}s)", cli.output.display());
        }
    } else {
        let pcm = generate_normal_full(ctx_ptr, &params.text)?;
        let mut result = pcm;
        apply_dsp(&mut result, cli.volume, cli.rate);
        let wav = audio::build_wav_in_memory(&result);
        fs::write(&cli.output, wav)?;
        if !cli.silent {
            let dur = result.len() as f32 / 24000.0;
            eprintln!("Wrote {} ({dur:.1}s)", cli.output.display());
        }
    }

    Ok(())
}

fn apply_dsp(pcm: &mut Vec<f32>, volume: Option<f32>, rate: Option<f32>) {
    if let Some(vol) = volume {
        unsafe {
            qwen_audio_apply_gain(pcm.as_mut_ptr(), pcm.len() as i32, vol);
        }
    }
    if let Some(r) = rate {
        let mut out: *mut f32 = std::ptr::null_mut();
        let mut out_n: i32 = 0;
        let ret = unsafe {
            qwen_audio_time_stretch(
                pcm.as_ptr(),
                pcm.len() as i32,
                r,
                24000,
                &mut out as *mut *mut f32,
                &mut out_n as *mut i32,
            )
        };
        if ret == 0 && !out.is_null() {
            let stretched = unsafe { std::slice::from_raw_parts(out, out_n as usize).to_vec() };
            unsafe {
                libc::free(out as *mut _);
            }
            *pcm = stretched;
        }
    }
}

fn has_markup(text: &str) -> bool {
    let c_text = std::ffi::CString::new(text).unwrap();
    unsafe { qwen_compose_has_markup(c_text.as_ptr() as *const std::os::raw::c_char) != 0 }
}

fn generate_normal_full(ctx: *mut qwen_tts_ctx, text: &str) -> Result<Vec<f32>> {
    let c_text = std::ffi::CString::new(text).unwrap();
    unsafe {
        qwen_ctx_set_stream(ctx, 0);
    }
    let mut out_samples: *mut f32 = std::ptr::null_mut();
    let mut out_n: i32 = 0;
    let ret = unsafe {
        qwen_tts_generate(
            ctx,
            c_text.as_ptr() as *const std::os::raw::c_char,
            &mut out_samples as *mut *mut f32,
            &mut out_n as *mut i32,
        )
    };
    if ret != 0 {
        anyhow::bail!("qwen_tts_generate failed with code {ret}");
    }
    let result = unsafe { std::slice::from_raw_parts(out_samples, out_n as usize).to_vec() };
    unsafe {
        libc::free(out_samples as *mut _);
    }
    Ok(result)
}

fn generate_compose_full(ctx: *mut qwen_tts_ctx, params: &ffi::GenerationParams) -> Result<Vec<f32>> {
    let c_text = std::ffi::CString::new(params.text.as_str()).unwrap();
    let c_lang =
        std::ffi::CString::new(server::lang_id_to_name(params.language_id)).unwrap();

    let mut spans: *mut qwen_cspan_t = std::ptr::null_mut();
    let mut n_spans: i32 = 0;
    let ret = unsafe {
        qwen_compose_parse(
            c_text.as_ptr() as *const std::os::raw::c_char,
            &mut spans as *mut *mut qwen_cspan_t,
            &mut n_spans as *mut i32,
        )
    };
    if ret != 0 {
        anyhow::bail!("Compose parse failed");
    }

    let mut out_audio: *mut f32 = std::ptr::null_mut();
    let mut out_n: i32 = 0;
    let ret = unsafe {
        qwen_compose_render_buffer(
            ctx,
            spans,
            n_spans,
            c_lang.as_ptr() as *const std::os::raw::c_char,
            0.5,
            &mut out_audio as *mut *mut f32,
            &mut out_n as *mut i32,
            0,
        )
    };
    unsafe {
        qwen_compose_free_spans(spans, n_spans);
    }
    if ret != 0 {
        anyhow::bail!("Compose render failed");
    }
    let result = unsafe { std::slice::from_raw_parts(out_audio, out_n as usize).to_vec() };
    unsafe {
        libc::free(out_audio as *mut _);
    }
    Ok(result)
}

fn synthesize_stream(
    ctx: *mut qwen_tts_ctx,
    params: &ffi::GenerationParams,
    output_path: &PathBuf,
    to_stdout: bool,
    chunk_frames: i32,
    use_compose: bool,
) -> Result<()> {
    let raw = ffi::RawCtx(ctx);
    let cancel = Arc::new(AtomicBool::new(false));
    let params_text = params.text.clone();
    let params_lang = server::lang_id_to_name(params.language_id).to_string();

    if to_stdout {
        let (tx, rx) = std::sync::mpsc::channel::<Vec<f32>>();

        std::thread::spawn(move || {
            if use_compose {
                generate_compose_stream_cb(raw.clone(), &params_text, &params_lang, tx, cancel);
            } else {
                generate_normal_stream_cb(raw.clone(), &params_text, chunk_frames, tx, cancel);
            }
        });

        let stdout = io::stdout();
        let mut handle = stdout.lock();
        for chunk in rx {
            let s16 = audio::f32_to_s16le(&chunk);
            handle.write_all(&s16)?;
            handle.flush()?;
        }
    } else {
        let (tx, rx) = std::sync::mpsc::channel::<Vec<f32>>();

        std::thread::spawn(move || {
            if use_compose {
                generate_compose_stream_cb(raw.clone(), &params_text, &params_lang, tx, cancel);
            } else {
                generate_normal_stream_cb(raw.clone(), &params_text, chunk_frames, tx, cancel);
            }
        });

        let all_samples: Vec<f32> = rx.into_iter().flatten().collect();
        let wav = audio::build_wav_in_memory(&all_samples);
        fs::write(output_path, wav)?;
        let dur = all_samples.len() as f32 / 24000.0;
        eprintln!("Wrote {} ({dur:.1}s)", output_path.display());
    }

    Ok(())
}

fn generate_normal_stream_cb(
    raw: ffi::RawCtx,
    text: &str,
    chunk_frames: i32,
    tx: std::sync::mpsc::Sender<Vec<f32>>,
    cancel: Arc<AtomicBool>,
) {
    let ctx = raw.0;
    struct CbData {
        tx: std::sync::mpsc::Sender<Vec<f32>>,
        cancel: Arc<AtomicBool>,
    }

    unsafe extern "C" fn audio_cb(
        samples: *const f32,
        n_samples: i32,
        userdata: *mut std::os::raw::c_void,
    ) -> i32 {
        if userdata.is_null() {
            return 0;
        }
        let data = &*(userdata as *const CbData);
        if data.cancel.load(Ordering::Relaxed) {
            return 1;
        }
        if n_samples > 0 && !samples.is_null() {
            let slice = std::slice::from_raw_parts(samples, n_samples as usize);
            let _ = data.tx.send(slice.to_vec());
        }
        0
    }

    let c_text = std::ffi::CString::new(text).unwrap();
    let cb_data = Box::new(CbData { tx, cancel });
    let cb_data_ptr = Box::into_raw(cb_data) as *mut std::os::raw::c_void;

    unsafe {
        qwen_ctx_set_stream(ctx, 1);
        qwen_ctx_set_stream_chunk_frames(ctx, chunk_frames);
        qwen_tts_set_audio_callback(ctx, Some(audio_cb), cb_data_ptr);
    }

    unsafe {
        qwen_tts_generate(
            ctx,
            c_text.as_ptr() as *const std::os::raw::c_char,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        qwen_ctx_clear_audio_callback(ctx);
    }

    unsafe {
        let _ = Box::from_raw(cb_data_ptr as *mut CbData);
    }
}

fn generate_compose_stream_cb(
    raw: ffi::RawCtx,
    text: &str,
    language: &str,
    tx: std::sync::mpsc::Sender<Vec<f32>>,
    cancel: Arc<AtomicBool>,
) {
    let ctx = raw.0;
    struct CbData {
        tx: std::sync::mpsc::Sender<Vec<f32>>,
        cancel: Arc<AtomicBool>,
    }

    unsafe extern "C" fn chunk_cb(
        pcm: *const f32,
        n: i32,
        user: *mut std::os::raw::c_void,
    ) {
        if user.is_null() {
            return;
        }
        let data = &*(user as *const CbData);
        if data.cancel.load(Ordering::Relaxed) {
            return;
        }
        if n > 0 && !pcm.is_null() {
            let slice = std::slice::from_raw_parts(pcm, n as usize);
            let _ = data.tx.send(slice.to_vec());
        }
    }

    let c_text = std::ffi::CString::new(text).unwrap();
    let c_lang = std::ffi::CString::new(language).unwrap();

    let mut spans: *mut qwen_cspan_t = std::ptr::null_mut();
    let mut n_spans: i32 = 0;
    let ret = unsafe {
        qwen_compose_parse(
            c_text.as_ptr() as *const std::os::raw::c_char,
            &mut spans as *mut *mut qwen_cspan_t,
            &mut n_spans as *mut i32,
        )
    };
    if ret != 0 {
        return;
    }

    let cb_data = Box::new(CbData { tx, cancel });
    let cb_data_ptr = Box::into_raw(cb_data) as *mut std::os::raw::c_void;

    unsafe {
        qwen_compose_render_stream(
            ctx,
            spans,
            n_spans,
            c_lang.as_ptr() as *const std::os::raw::c_char,
            0.5,
            Some(chunk_cb),
            cb_data_ptr,
            0,
        );
        qwen_compose_free_spans(spans, n_spans);
        let _ = Box::from_raw(cb_data_ptr as *mut CbData);
    }
}

fn run_server(
    model_dir: &str,
    port: u16,
    silent: bool,
    use_int8: bool,
    use_int4: bool,
    ref_audio: Option<String>,
    ref_text: Option<String>,
    idle_secs: Option<u64>,
) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let voice_clone = ref_audio.is_some();

        let config = server::ServerConfig {
            model_dir: model_dir.to_string(),
            silent,
            use_int8,
            use_int4,
            ref_audio_path: ref_audio.clone(),
            ref_text: ref_text.clone(),
            voice_clone,
            idle_secs,
        };

        let state = server::AppState {
            config: Arc::new(config),
            ctx: Arc::new(std::sync::Mutex::new(None)),
        };

        if !silent {
            eprintln!("Starting HTTP server on http://localhost:{port}");
            eprintln!("  GET  /v1/health");
            eprintln!("  GET  /v1/speakers");
            eprintln!("  POST /v1/tts");
            eprintln!("  POST /v1/tts/stream");
            eprintln!("  POST /v1/audio/speech");
        }

        let app = server::router(state);

        let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
            .await
            .unwrap();

        axum::serve(listener, app).await.unwrap();
    });
}
