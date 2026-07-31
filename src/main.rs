//! Qwen3-TTS Rust Server
//! 
//! This project wraps the C-based Qwen3-TTS inference engine with a Rust server
//! implementation using tokio and axum for async HTTP handling.

mod ffi;

use std::sync::Arc;
use tokio::sync::Mutex;

use axum::{
    extract::State,
    http::StatusCode,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::ffi::qwen_tts::{self, TtsContext, TtsParams, TtsResult as TtsAudioResult};

/// Shared application state
struct AppState {
    tts_context: Mutex<TtsContext>,
}

// SAFETY: TtsContext is Send but not Sync (wrapped in Mutex for thread-safe access)
unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}

#[derive(Debug, Deserialize)]
struct TtsRequest {
    text: String,
    #[serde(default = "default_speaker")]
    speaker: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default)]
    temperature: Option<f32>,
    #[serde(default)]
    top_k: Option<i32>,
    #[serde(default)]
    top_p: Option<f32>,
    #[serde(default)]
    speed: Option<f32>,
    #[serde(default)]
    volume: Option<f32>,
}

fn default_speaker() -> String {
    "ryan".to_string()
}

fn default_language() -> String {
    "English".to_string()
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
}

#[derive(Debug, Serialize)]
struct SpeakerInfo {
    name: String,
    language: String,
    gender: String,
}

#[derive(Debug, Serialize)]
struct SpeakersResponse {
    speakers: Vec<SpeakerInfo>,
}

/// Convert TtsError to HTTP error response
fn tts_error_to_http_error(err: qwen_tts::TtsError) -> (StatusCode, Json<ErrorResponse>) {
    let status = match err.code {
        qwen_tts::TtsErrorCode::InvalidArgument => StatusCode::BAD_REQUEST,
        qwen_tts::TtsErrorCode::SpeakerNotFound | qwen_tts::TtsErrorCode::LanguageNotFound => StatusCode::BAD_REQUEST,
        qwen_tts::TtsErrorCode::ModelNotFound => StatusCode::INTERNAL_SERVER_ERROR,
        qwen_tts::TtsErrorCode::InitializationFailed => StatusCode::INTERNAL_SERVER_ERROR,
        qwen_tts::TtsErrorCode::InferenceFailed => StatusCode::INTERNAL_SERVER_ERROR,
        qwen_tts::TtsErrorCode::OutOfMemory => StatusCode::INSUFFICIENT_STORAGE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    
    (
        status,
        Json(ErrorResponse {
            error: format!("{}: {}", err.code, err.message),
        }),
    )
}

/// Health check endpoint
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

/// List available speakers
async fn list_speakers() -> Json<SpeakersResponse> {
    let speakers = vec![
        SpeakerInfo { name: "ryan".to_string(), language: "English".to_string(), gender: "male".to_string() },
        SpeakerInfo { name: "aiden".to_string(), language: "English".to_string(), gender: "male".to_string() },
        SpeakerInfo { name: "vivian".to_string(), language: "Chinese".to_string(), gender: "female".to_string() },
        SpeakerInfo { name: "serena".to_string(), language: "Chinese".to_string(), gender: "female".to_string() },
        SpeakerInfo { name: "uncle_fu".to_string(), language: "Chinese".to_string(), gender: "male".to_string() },
        SpeakerInfo { name: "dylan".to_string(), language: "Chinese".to_string(), gender: "male".to_string() },
        SpeakerInfo { name: "eric".to_string(), language: "Chinese".to_string(), gender: "male".to_string() },
        SpeakerInfo { name: "ono_anna".to_string(), language: "Japanese".to_string(), gender: "female".to_string() },
        SpeakerInfo { name: "sohee".to_string(), language: "Korean".to_string(), gender: "female".to_string() },
    ];
    Json(SpeakersResponse { speakers })
}

/// Generate speech from text - returns WAV audio
async fn generate_tts(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TtsRequest>,
) -> Result<Response<'static>, (StatusCode, Json<ErrorResponse>)> {
    // Build parameters
    let mut params = TtsParams::new()
        .with_speaker(&req.speaker)
        .with_language(&req.language);
    
    if let Some(temp) = req.temperature {
        params = params.with_temperature(temp);
    }
    if let Some(k) = req.top_k {
        params = params.with_top_k(k);
    }
    if let Some(p) = req.top_p {
        params = params.with_top_p(p);
    }
    if let Some(speed) = req.speed {
        params = params.with_speed(speed);
    }
    if let Some(volume) = req.volume {
        params = params.with_volume(volume);
    }
    
    // Lock the context for thread-safe access
    let ctx_guard = state.tts_context.lock().await;
    
    // Generate audio
    let result = ctx_guard.generate_with_params(&req.text, &params)
        .map_err(tts_error_to_http_error)?;
    
    drop(ctx_guard); // Release lock early
    
    // Create WAV data
    let wav_data = result.to_wav();
    
    // Build HTTP response with WAV content
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "audio/wav")
        .header("Content-Length", wav_data.len())
        .header("X-Audio-Duration", result.duration().to_string())
        .body(axum::body::Body::from(wav_data))
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: format!("Failed to build response: {}", e) }),
        ))?;
    
    Ok(response)
}

/// OpenAI-compatible TTS endpoint
async fn openai_speech(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TtsRequest>,
) -> Result<Response<'static>, (StatusCode, Json<ErrorResponse>)> {
    generate_tts(State(state), Json(req)).await
}

#[tokio::main]
async fn main() {
    println!("Qwen3-TTS Rust Server starting...");
    
    // Initialize the TTS context
    let model_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "./qwen3-tts-model".to_string());
    
    let ctx = TtsContext::new(&model_dir)
        .unwrap_or_else(|e| {
            eprintln!("Failed to initialize TTS context. Model directory: {}", model_dir);
            eprintln!("Error: {}", e);
            eprintln!("Please ensure the model files are present.");
            std::process::exit(1);
        });
    
    println!("TTS context initialized successfully");
    println!("  Sample rate: {} Hz", qwen_tts::QWEN_TTS_SAMPLE_RATE);
    println!("  Default speaker: Ryan");
    println!("  Default language: English");
    
    // Create shared state
    let state = Arc::new(AppState {
        tts_context: Mutex::new(ctx),
    });
    
    // Build router
    let app = Router::new()
        .route("/v1/health", get(health_check))
        .route("/v1/speakers", get(list_speakers))
        .route("/v1/tts", post(generate_tts))
        .route("/v1/audio/speech", post(openai_speech))
        .with_state(state);
    
    // Run server
    let addr = "0.0.0.0:8080";
    println!("Server listening on {}", addr);
    println!("\nAPI Endpoints:");
    println!("  GET  /v1/health       - Health check");
    println!("  GET  /v1/speakers     - List available speakers");
    println!("  POST /v1/tts          - Generate speech (WAV output)");
    println!("  POST /v1/audio/speech - OpenAI-compatible TTS API");
    println!("\nExample usage:");
    println!("  curl -X POST http://localhost:8080/v1/tts \\");
    println!("    -H \"Content-Type: application/json\" \\");
    println!("    -d '{{\"text\": \"Hello, world!\", \"speaker\": \"ryan\"}}' \\");
    println!("    --output output.wav");
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
