use crate::bindings::*;
use crate::ffi;
use crate::audio;
use anyhow::Result;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub ctx: Arc<Mutex<Option<ffi::RawCtx>>>,
}

#[derive(Clone)]
pub struct ServerConfig {
    pub model_dir: String,
    pub silent: bool,
    pub use_int8: bool,
    pub use_int4: bool,
    pub ref_audio_path: Option<String>,
    pub ref_text: Option<String>,
    pub voice_clone: bool,
    #[allow(dead_code)]
    pub idle_secs: Option<u64>,
}

#[derive(Deserialize)]
struct TtsRequest {
    text: Option<String>,
    input: Option<String>,
    speaker: Option<String>,
    voice: Option<String>,
    language: Option<String>,
    instruct: Option<String>,
    voice_design: Option<String>,
    temperature: Option<f32>,
    top_k: Option<i32>,
    top_p: Option<f32>,
    rep_penalty: Option<f32>,
    seed: Option<u32>,
    emotion: Option<String>,
    volume: Option<f32>,
    rate: Option<f32>,
    #[serde(rename = "model")]
    _model: Option<String>,
    #[serde(rename = "response_format")]
    _response_format: Option<String>,
}

#[derive(Serialize)]
struct Speaker {
    name: String,
    id: i32,
}

#[derive(Serialize)]
struct SpeakerResponse {
    speakers: Vec<Speaker>,
    sample_rate: u32,
}

const SPEAKERS: &[(&str, i32)] = &[
    ("ryan", 3061),
    ("aiden", 2861),
    ("vivian", 3065),
    ("serena", 3066),
    ("uncle_fu", 3010),
    ("dylan", 2878),
    ("eric", 2875),
    ("ono_anna", 2873),
    ("sohee", 2864),
];

fn resolve_text(req: &TtsRequest) -> Option<&str> {
    req.text.as_deref().or(req.input.as_deref())
}

fn resolve_speaker_name(req: &TtsRequest) -> Option<&str> {
    req.speaker.as_deref().or(req.voice.as_deref())
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/speakers", get(speakers_list))
        .route("/v1/tts", post(tts_full))
        .route("/v1/tts/stream", post(tts_stream))
        .route("/v1/audio/speech", post(tts_openai))
        .route("/health", get(health))
        .route("/speakers", get(speakers_list))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    let guard = state.ctx.lock().unwrap();
    let loaded = guard.is_some();
    drop(guard);
    Json(serde_json::json!({
        "status": "ok",
        "loaded": loaded,
    }))
}

async fn speakers_list() -> Json<SpeakerResponse> {
    let speakers: Vec<Speaker> = SPEAKERS
        .iter()
        .map(|(name, id)| Speaker {
            name: name.to_string(),
            id: *id,
        })
        .collect();
    Json(SpeakerResponse {
        speakers,
        sample_rate: 24000,
    })
}

async fn tts_full(
    State(state): State<AppState>,
    Json(req): Json<TtsRequest>,
) -> Response {
    match synthesize_full(&state, req).await {
        Ok(wav) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "audio/wav")
            .header("X-Sample-Rate", "24000")
            .body(axum::body::Body::from(wav))
            .unwrap(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("{e}")})),
        )
            .into_response(),
    }
}

async fn tts_stream(
    State(state): State<AppState>,
    Json(req): Json<TtsRequest>,
) -> Response {
    let text = resolve_text(&req).unwrap_or("").to_string();
    if text.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Missing text/input"})),
        )
            .into_response();
    }
    if text.len() > 8192 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Text too long"})),
        )
            .into_response();
    }

    let config = state.config.clone();
    let params = build_params(&config, &req, text);
    let ctx_arc = state.ctx.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let mut guard = ctx_arc.lock().unwrap();
        let raw_ctx = ensure_loaded_sync(&config, &mut guard)?;
        let ctx_ptr = raw_ctx.0;

        unsafe {
            ffi::reset_context_state(ctx_ptr);
            ffi::apply_params_to_context(ctx_ptr, &params);
        }

        if let Some(ref emotion) = params.emotion {
            let lang = lang_id_to_name(params.language_id);
            unsafe {
                ffi::apply_emotion(ctx_ptr, emotion, lang);
            }
        }

        let pcm = if has_markup(&params.text) {
            generate_compose_full(ctx_ptr, &params)?
        } else {
            generate_normal_full(ctx_ptr, &params.text)?
        };
        drop(guard);

        Ok(audio::f32_to_s16le(&pcm))
    })
    .await;

    match result {
        Ok(Ok(data)) => {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, "audio/pcm".parse().unwrap());
            headers.insert("X-Sample-Rate", "24000".parse().unwrap());
            headers.insert("X-Sample-Format", "s16le".parse().unwrap());
            headers.insert("X-Channels", "1".parse().unwrap());
            (StatusCode::OK, headers, data).into_response()
        }
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Generation failed"})),
        )
            .into_response(),
    }
}

async fn tts_openai(
    State(state): State<AppState>,
    Json(req): Json<TtsRequest>,
) -> Response {
    tts_full(State(state), Json(req)).await
}

async fn synthesize_full(state: &AppState, req: TtsRequest) -> Result<Vec<u8>> {
    let text = resolve_text(&req).unwrap_or("").to_string();
    if text.is_empty() {
        anyhow::bail!("Missing text/input");
    }
    if text.len() > 8192 {
        anyhow::bail!("Text too long");
    }

    let params = build_params(&state.config, &req, text);
    let config = state.config.clone();
    let ctx_arc = state.ctx.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let mut guard = ctx_arc.lock().unwrap();
        let raw_ctx = ensure_loaded_sync(&config, &mut guard)?;
        let ctx_ptr = raw_ctx.0;

        unsafe {
            ffi::reset_context_state(ctx_ptr);
            ffi::apply_params_to_context(ctx_ptr, &params);
        }

        if let Some(ref emotion) = params.emotion {
            let lang = lang_id_to_name(params.language_id);
            unsafe {
                ffi::apply_emotion(ctx_ptr, emotion, lang);
            }
        }

        let mut pcm = if has_markup(&params.text) {
            generate_compose_full(ctx_ptr, &params)?
        } else {
            generate_normal_full(ctx_ptr, &params.text)?
        };

        if let Some(vol) = params.volume {
            unsafe {
                qwen_audio_apply_gain(pcm.as_mut_ptr(), pcm.len() as i32, vol);
            }
        }
        if let Some(r) = params.rate {
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
                let stretched =
                    unsafe { std::slice::from_raw_parts(out, out_n as usize).to_vec() };
                unsafe {
                    libc::free(out as *mut _);
                }
                pcm = stretched;
            }
        }

        Ok(audio::build_wav_in_memory(&pcm))
    })
    .await??;

    Ok(result)
}

fn has_markup(text: &str) -> bool {
    let c_text = std::ffi::CString::new(text).unwrap();
    unsafe {
        qwen_compose_has_markup(c_text.as_ptr() as *const std::os::raw::c_char) != 0
    }
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
        std::ffi::CString::new(lang_id_to_name(params.language_id)).unwrap();

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

#[allow(dead_code)]
fn generate_normal_stream_std(
    ctx: *mut qwen_tts_ctx,
    text: &str,
    tx: std::sync::mpsc::Sender<Vec<f32>>,
) {
    struct CbData {
        tx: std::sync::Mutex<std::sync::mpsc::Sender<Vec<f32>>>,
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
        if n_samples > 0 && !samples.is_null() {
            let slice = std::slice::from_raw_parts(samples, n_samples as usize);
            if let Ok(tx) = data.tx.lock() {
                let _ = tx.send(slice.to_vec());
            }
        }
        0
    }

    let c_text = std::ffi::CString::new(text).unwrap();
    let cb_data = Box::new(CbData {
        tx: std::sync::Mutex::new(tx),
    });
    let cb_data_ptr = Box::into_raw(cb_data);

    unsafe {
        qwen_ctx_set_stream(ctx, 1);
        qwen_ctx_set_stream_chunk_frames(ctx, 10);
        qwen_tts_set_audio_callback(
            ctx,
            Some(audio_cb),
            cb_data_ptr as *mut std::os::raw::c_void,
        );
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
        let _ = Box::from_raw(cb_data_ptr);
    }
}

#[allow(dead_code)]
fn generate_compose_stream_std(
    ctx: *mut qwen_tts_ctx,
    params: &ffi::GenerationParams,
    tx: std::sync::mpsc::Sender<Vec<f32>>,
) {
    struct CbData {
        tx: std::sync::Mutex<std::sync::mpsc::Sender<Vec<f32>>>,
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
        if n > 0 && !pcm.is_null() {
            let slice = std::slice::from_raw_parts(pcm, n as usize);
            if let Ok(tx) = data.tx.lock() {
                let _ = tx.send(slice.to_vec());
            }
        }
    }

    let c_text = std::ffi::CString::new(params.text.as_str()).unwrap();
    let c_lang =
        std::ffi::CString::new(lang_id_to_name(params.language_id)).unwrap();

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

    let cb_data = Box::new(CbData {
        tx: std::sync::Mutex::new(tx),
    });
    let cb_data_ptr = Box::into_raw(cb_data);

    unsafe {
        qwen_compose_render_stream(
            ctx,
            spans,
            n_spans,
            c_lang.as_ptr() as *const std::os::raw::c_char,
            0.5,
            Some(chunk_cb),
            cb_data_ptr as *mut std::os::raw::c_void,
            0,
        );
        qwen_compose_free_spans(spans, n_spans);
        let _ = Box::from_raw(cb_data_ptr);
    }
}

fn build_params(
    config: &ServerConfig,
    req: &TtsRequest,
    text: String,
) -> ffi::GenerationParams {
    let speaker_id = resolve_speaker_name(req)
        .and_then(|s| {
            let id = unsafe { ffi::speaker_id(s) };
            if id >= 0 {
                Some(id)
            } else {
                None
            }
        })
        .unwrap_or(3061);

    let language_id = req
        .language
        .as_deref()
        .and_then(|l| {
            let id = unsafe { ffi::language_id(l) };
            if id >= 0 {
                Some(id)
            } else {
                None
            }
        })
        .unwrap_or(2050);

    let voice_design = req
        .voice_design
        .as_deref()
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    ffi::GenerationParams {
        text,
        speaker_id,
        language_id,
        temperature: req.temperature.unwrap_or(0.5),
        top_k: req.top_k.unwrap_or(50),
        top_p: req.top_p.unwrap_or(1.0),
        rep_penalty: req.rep_penalty.unwrap_or(1.05),
        seed: req.seed.unwrap_or(0),
        max_tokens: 8192,
        instruct: req.instruct.clone(),
        voice_design,
        voice_clone: config.voice_clone,
        xvector_only: false,
        ref_audio_path: config.ref_audio_path.clone(),
        ref_text: config.ref_text.clone(),
        cp_roughness: 0.0,
        emotion: req.emotion.clone(),
        volume: req.volume,
        rate: req.rate,
    }
}

fn ensure_loaded_sync(
    config: &ServerConfig,
    guard: &mut std::sync::MutexGuard<Option<ffi::RawCtx>>,
) -> Result<ffi::RawCtx> {
    if let Some(raw) = **guard {
        return Ok(raw);
    }

    let ctx = unsafe {
        ffi::Context::load(
            &config.model_dir,
            config.silent,
            config.use_int8,
            config.use_int4,
        )
    };
    let ctx = ctx.ok_or_else(|| anyhow::anyhow!("Failed to load model"))?;
    let ptr = ctx.as_ptr();
    std::mem::forget(ctx);

    if config.voice_clone {
        unsafe {
            if let Some(ref ref_audio) = config.ref_audio_path {
                let c_audio = std::ffi::CString::new(ref_audio.as_str()).unwrap();
                let c_ref_text = config.ref_text.as_deref()
                    .map(|t| std::ffi::CString::new(t).unwrap());
                let ref_text_ptr = c_ref_text.as_ref()
                    .map(|s| s.as_ptr())
                    .unwrap_or(std::ptr::null());

                let ret = qwen_ctx_init_voice_clone(
                    ptr,
                    c_audio.as_ptr() as *const std::os::raw::c_char,
                    ref_text_ptr as *const std::os::raw::c_char,
                    0,
                    if config.silent { 1 } else { 0 },
                );
                if ret != 0 {
                    ffi::Context::cleanup_raw(ptr);
                    return Err(anyhow::anyhow!("Failed to initialize voice clone"));
                }
            }
        }
    }

    let raw = ffi::RawCtx(ptr);
    **guard = Some(raw);
    Ok(raw)
}

pub fn lang_id_to_name(id: i32) -> &'static str {
    match id {
        2055 => "Chinese",
        2050 => "English",
        2058 => "Japanese",
        2064 => "Korean",
        2053 => "German",
        2061 => "French",
        2069 => "Russian",
        2071 => "Portuguese",
        2054 => "Spanish",
        2070 => "Italian",
        _ => "English",
    }
}
