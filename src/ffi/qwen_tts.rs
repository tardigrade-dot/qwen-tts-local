//! FFI bindings for qwen_tts.h - Core TTS context and inference

use std::ffi::{c_char, c_int, c_float, c_void, c_uint, c_size_t};
use std::ffi::CString;
use std::ptr;
use std::fmt;

// Constants from qwen_tts.h
pub const QWEN_TTS_SAMPLE_RATE: c_int = 24000;
pub const QWEN_TTS_FRAME_RATE: c_float = 12.5;
pub const QWEN_TTS_HOP_SAMPLES: c_int = 1920;

pub const QWEN_TTS_MAX_TALKER_LAYERS: c_int = 28;
pub const QWEN_TTS_MAX_CP_LAYERS: c_int = 5;
pub const QWEN_TTS_MAX_DECODER_LAYERS: c_int = 8;

pub const QWEN_TTS_TEXT_VOCAB_SIZE: c_int = 151936;
pub const QWEN_TTS_CODEC_VOCAB_SIZE: c_int = 3072;
pub const QWEN_TTS_CODEBOOK_SIZE: c_int = 2048;
pub const QWEN_TTS_NUM_CODEBOOKS: c_int = 16;
pub const QWEN_TTS_CODEBOOK_DIM: c_int = 256;

// Special token IDs
pub const QWEN_TTS_TOK_IM_START: c_int = 151644;
pub const QWEN_TTS_TOK_IM_END: c_int = 151645;
pub const QWEN_TTS_TOK_ENDOFTEXT: c_int = 151643;
pub const QWEN_TTS_TTS_BOS: c_int = 151672;
pub const QWEN_TTS_TTS_EOS: c_int = 151673;
pub const QWEN_TTS_TTS_PAD: c_int = 151671;

// Codec special tokens
pub const QWEN_TTS_CODEC_PAD: c_int = 2148;
pub const QWEN_TTS_CODEC_BOS: c_int = 2149;
pub const QWEN_TTS_CODEC_EOS: c_int = 2150;

// Language IDs
pub const QWEN_TTS_LANG_CHINESE: c_int = 2055;
pub const QWEN_TTS_LANG_ENGLISH: c_int = 2050;
pub const QWEN_TTS_LANG_JAPANESE: c_int = 2058;
pub const QWEN_TTS_LANG_KOREAN: c_int = 2064;

// Speaker IDs (CustomVoice)
pub const QWEN_TTS_SPEAKER_SERENA: c_int = 3066;
pub const QWEN_TTS_SPEAKER_VIVIAN: c_int = 3065;
pub const QWEN_TTS_SPEAKER_UNCLE_FU: c_int = 3010;
pub const QWEN_TTS_SPEAKER_RYAN: c_int = 3061;
pub const QWEN_TTS_SPEAKER_AIDEN: c_int = 2861;

// ============================================================================
// Error codes
// ============================================================================

/// Error codes returned by TTS functions
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtsErrorCode {
    Success = 0,
    InvalidArgument = -1,
    OutOfMemory = -2,
    ModelNotFound = -3,
    InitializationFailed = -4,
    InferenceFailed = -5,
    AudioEncodingFailed = -6,
    SpeakerNotFound = -7,
    LanguageNotFound = -8,
    InternalError = -99,
}

impl fmt::Display for TtsErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TtsErrorCode::Success => write!(f, "Success"),
            TtsErrorCode::InvalidArgument => write!(f, "Invalid argument"),
            TtsErrorCode::OutOfMemory => write!(f, "Out of memory"),
            TtsErrorCode::ModelNotFound => write!(f, "Model not found"),
            TtsErrorCode::InitializationFailed => write!(f, "Initialization failed"),
            TtsErrorCode::InferenceFailed => write!(f, "Inference failed"),
            TtsErrorCode::AudioEncodingFailed => write!(f, "Audio encoding failed"),
            TtsErrorCode::SpeakerNotFound => write!(f, "Speaker not found"),
            TtsErrorCode::LanguageNotFound => write!(f, "Language not found"),
            TtsErrorCode::InternalError => write!(f, "Internal error"),
        }
    }
}

impl std::error::Error for TtsErrorCode {}

/// Result type for TTS operations
pub type TtsResult<T> = Result<T, TtsError>;

/// Error information for TTS operations
#[derive(Debug)]
pub struct TtsError {
    pub code: TtsErrorCode,
    pub message: String,
}

impl TtsError {
    pub fn new(code: TtsErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    
    pub fn from_code(code: TtsErrorCode) -> Self {
        Self {
            code,
            message: code.to_string(),
        }
    }
}

impl fmt::Display for TtsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for TtsError {}

impl From<TtsErrorCode> for TtsError {
    fn from(code: TtsErrorCode) -> Self {
        TtsError::from_code(code)
    }
}

// ============================================================================
// Context parameters structure
// ============================================================================

/// Parameters for configuring TTS generation
#[derive(Debug, Clone)]
pub struct TtsParams {
    pub speaker_id: c_int,
    pub language_id: c_int,
    pub temperature: c_float,
    pub top_k: c_int,
    pub top_p: c_float,
    pub speed: c_float,
    pub volume: c_float,
}

impl Default for TtsParams {
    fn default() -> Self {
        Self {
            speaker_id: QWEN_TTS_SPEAKER_RYAN,
            language_id: QWEN_TTS_LANG_ENGLISH,
            temperature: 0.7,
            top_k: 50,
            top_p: 0.95,
            speed: 1.0,
            volume: 1.0,
        }
    }
}

impl TtsParams {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn with_speaker(mut self, speaker: &str) -> Self {
        if let Some(id) = get_speaker_id(speaker) {
            self.speaker_id = id;
        }
        self
    }
    
    pub fn with_language(mut self, language: &str) -> Self {
        if let Some(id) = get_language_id(language) {
            self.language_id = id;
        }
        self
    }
    
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp.clamp(0.0, 2.0);
        self
    }
    
    pub fn with_top_k(mut self, k: i32) -> Self {
        self.top_k = k.max(1);
        self
    }
    
    pub fn with_top_p(mut self, p: f32) -> Self {
        self.top_p = p.clamp(0.0, 1.0);
        self
    }
    
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed.clamp(0.5, 2.0);
        self
    }
    
    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume.clamp(0.0, 2.0);
        self
    }
}

// ============================================================================
// Opaque context type
// ============================================================================

#[repr(C)]
pub struct qwen_tts_ctx {
    _private: [u8; 0],
}

pub type qwen_tts_ctx_t = qwen_tts_ctx;

// ============================================================================
// Callback types
// ============================================================================

/// Audio callback type: called with samples during streaming
/// Returns 0 on success, non-zero to stop
pub type qwen_tts_audio_cb = Option<unsafe extern "C" fn(
    samples: *const c_float,
    n_samples: c_int,
    userdata: *mut c_void,
) -> c_int>;

/// Progress callback type: called with generation progress
/// progress: 0.0 to 1.0
/// Returns 0 to continue, non-zero to cancel
pub type qwen_tts_progress_cb = Option<unsafe extern "C" fn(
    progress: c_float,
    userdata: *mut c_void,
) -> c_int>;

// ============================================================================
// Core API functions (from qwen_tts.c)
// ============================================================================

extern "C" {
    /// Initialize the TTS engine with a model directory
    /// Returns 0 on success, negative error code on failure
    pub fn qwen_tts_init(model_dir: *const c_char, ctx: *mut *mut qwen_tts_ctx_t) -> c_int;
    
    /// Free the TTS context and all associated resources
    pub fn qwen_tts_free(ctx: *mut qwen_tts_ctx_t);
    
    /// Set context parameters before generation
    /// Returns 0 on success, negative error code on failure
    pub fn qwen_tts_set_params(
        ctx: *mut qwen_tts_ctx_t,
        speaker_id: c_int,
        language_id: c_int,
        temperature: c_float,
        top_k: c_int,
        top_p: c_float,
    ) -> c_int;
    
    /// Set additional synthesis parameters (speed, volume, etc.)
    /// Returns 0 on success, negative error code on failure
    pub fn qwen_tts_set_synthesis_params(
        ctx: *mut qwen_tts_ctx_t,
        speed: c_float,
        volume: c_float,
        emotion: *const c_char,
    ) -> c_int;
    
    /// Generate speech from text synchronously
    /// Returns allocated samples array (caller must free via qwen_tts_free_samples)
    /// Sets n_samples on success
    /// Returns NULL on error
    pub fn qwen_tts_generate(
        ctx: *mut qwen_tts_ctx_t,
        text: *const c_char,
        n_samples: *mut c_int,
    ) -> *mut c_float;
    
    /// Generate speech with parameters synchronously
    /// Convenience function that sets params before generation
    /// Returns allocated samples array (caller must free)
    /// Returns NULL on error
    pub fn qwen_tts_generate_with_params(
        ctx: *mut qwen_tts_ctx_t,
        text: *const c_char,
        speaker_id: c_int,
        language_id: c_int,
        temperature: c_float,
        top_k: c_int,
        top_p: c_float,
        n_samples: *mut c_int,
    ) -> *mut c_float;
    
    /// Generate speech with streaming callback
    /// Returns 0 on success, negative error code on failure
    pub fn qwen_tts_generate_stream(
        ctx: *mut qwen_tts_ctx_t,
        text: *const c_char,
        cb: qwen_tts_audio_cb,
        userdata: *mut c_void,
    ) -> c_int;
    
    /// Generate speech with streaming callback and progress reporting
    /// Returns 0 on success, negative error code on failure
    pub fn qwen_tts_generate_stream_ex(
        ctx: *mut qwen_tts_ctx_t,
        text: *const c_char,
        audio_cb: qwen_tts_audio_cb,
        progress_cb: qwen_tts_progress_cb,
        userdata: *mut c_void,
    ) -> c_int;
    
    /// Free samples allocated by qwen_tts_generate
    pub fn qwen_tts_free_samples(samples: *mut c_float);
    
    /// Get speaker ID by name, or -1 if not found
    pub fn qwen_tts_speaker_id(name: *const c_char) -> c_int;
    
    /// Get language ID by name, or -1 if not found  
    pub fn qwen_tts_language_id(name: *const c_char) -> c_int;
    
    /// Get last error message (thread-local)
    pub fn qwen_tts_get_last_error() -> *const c_char;
    
    /// Clear last error (thread-local)
    pub fn qwen_tts_clear_error();
}

// ============================================================================
// Safe Rust wrappers for FFI functions
// ============================================================================

/// Safe wrapper for getting speaker ID
pub fn get_speaker_id(name: &str) -> Option<c_int> {
    let c_name = CString::new(name).ok()?;
    let id = unsafe { qwen_tts_speaker_id(c_name.as_ptr()) };
    if id >= 0 { Some(id) } else { None }
}

/// Safe wrapper for getting language ID
pub fn get_language_id(name: &str) -> Option<c_int> {
    let c_name = CString::new(name).ok()?;
    let id = unsafe { qwen_tts_language_id(c_name.as_ptr()) };
    if id >= 0 { Some(id) } else { None }
}

/// Get the last error message from the C library
pub fn get_last_error() -> Option<String> {
    unsafe {
        let err_ptr = qwen_tts_get_last_error();
        if err_ptr.is_null() {
            None
        } else {
            std::ffi::CStr::from_ptr(err_ptr).to_str().ok().map(|s| s.to_string())
        }
    }
}

/// Clear the last error
pub fn clear_error() {
    unsafe { qwen_tts_clear_error() }
}

// ============================================================================
// High-level safe API
// ============================================================================

/// Safe wrapper for TTS context management
pub struct TtsContext {
    ctx: *mut qwen_tts_ctx_t,
    params: TtsParams,
}

unsafe impl Send for TtsContext {}
// Note: Not Sync because the underlying C context may not be thread-safe
// Users should use Arc<Mutex<TtsContext>> for shared access

impl TtsContext {
    /// Initialize a new TTS context with the given model directory
    pub fn new(model_dir: &str) -> TtsResult<Self> {
        let c_model_dir = CString::new(model_dir)
            .map_err(|_| TtsError::new(TtsErrorCode::InvalidArgument, "Invalid model directory path"))?;
        
        let mut ctx: *mut qwen_tts_ctx_t = ptr::null_mut();
        let result = unsafe { qwen_tts_init(c_model_dir.as_ptr(), &mut ctx) };
        
        if result != 0 || ctx.is_null() {
            let error_msg = get_last_error().unwrap_or_else(|| "Unknown initialization error".to_string());
            return Err(TtsError::new(
                match result {
                    -2 => TtsErrorCode::OutOfMemory,
                    -3 => TtsErrorCode::ModelNotFound,
                    _ => TtsErrorCode::InitializationFailed,
                },
                error_msg,
            ));
        }
        
        Ok(Self {
            ctx,
            params: TtsParams::default(),
        })
    }
    
    /// Get the raw context pointer (for advanced usage)
    pub fn as_raw_ptr(&self) -> *mut qwen_tts_ctx_t {
        self.ctx
    }
    
    /// Update context parameters
    pub fn set_params(&mut self, params: TtsParams) -> TtsResult<()> {
        let result = unsafe {
            qwen_tts_set_params(
                self.ctx,
                params.speaker_id,
                params.language_id,
                params.temperature,
                params.top_k,
                params.top_p,
            )
        };
        
        if result != 0 {
            return Err(TtsError::from_code(TtsErrorCode::InvalidArgument));
        }
        
        // Also set synthesis params if available
        unsafe {
            qwen_tts_set_synthesis_params(self.ctx, params.speed, params.volume, ptr::null());
        }
        
        self.params = params;
        Ok(())
    }
    
    /// Get current parameters
    pub fn get_params(&self) -> &TtsParams {
        &self.params
    }
    
    /// Generate speech synchronously
    pub fn generate(&self, text: &str) -> TtsResult<TtsResult> {
        self.generate_with_params_internal(None)
    }
    
    /// Generate speech with custom parameters
    pub fn generate_with_params(&self, text: &str, params: &TtsParams) -> TtsResult<TtsResult> {
        // Temporarily set params
        let result = unsafe {
            qwen_tts_set_params(
                self.ctx,
                params.speaker_id,
                params.language_id,
                params.temperature,
                params.top_k,
                params.top_p,
            )
        };
        
        if result != 0 {
            return Err(TtsError::from_code(TtsErrorCode::InvalidArgument));
        }
        
        self.generate_with_params_internal(Some(params))
    }
    
    fn generate_with_params_internal(&self, _params_override: Option<&TtsParams>) -> TtsResult<TtsResult> {
        let c_text = CString::new(text)
            .map_err(|_| TtsError::new(TtsErrorCode::InvalidArgument, "Invalid text (contains null byte)"))?;
        
        let mut n_samples: c_int = 0;
        let samples_ptr = unsafe {
            qwen_tts_generate(self.ctx, c_text.as_ptr(), &mut n_samples)
        };
        
        if samples_ptr.is_null() {
            let error_msg = get_last_error().unwrap_or_else(|| "Generation failed".to_string());
            return Err(TtsError::new(TtsErrorCode::InferenceFailed, error_msg));
        }
        
        // Safely copy samples to Vec
        let samples = unsafe {
            let slice = std::slice::from_raw_parts(samples_ptr, n_samples as usize);
            slice.to_vec()
        };
        
        // Free the C-allocated buffer
        unsafe {
            qwen_tts_free_samples(samples_ptr);
        }
        
        Ok(TtsResult { samples })
    }
}

impl Drop for TtsContext {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe {
                qwen_tts_free(self.ctx);
            }
        }
    }
}

/// Result of synchronous TTS generation
pub struct TtsResult {
    pub samples: Vec<f32>,
}

impl TtsResult {
    /// Convert samples to WAV format bytes
    pub fn to_wav(&self) -> Vec<u8> {
        self.to_wav_with_sample_rate(QWEN_TTS_SAMPLE_RATE as u32)
    }
    
    /// Convert samples to WAV format with custom sample rate
    pub fn to_wav_with_sample_rate(&self, sample_rate: u32) -> Vec<u8> {
        let bits = 16u16;
        let channels = 1u16;
        let data_size = (self.samples.len() * channels as usize * (bits / 8) as usize) as u32;
        let file_size = 36 + data_size;
        
        let mut wav = Vec::with_capacity(44 + self.samples.len() * 2);
        
        // RIFF header
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&file_size.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        let byte_rate = sample_rate * channels as u32 * (bits as u32 / 8);
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        let block_align = channels * (bits / 8);
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        
        // PCM samples (convert f32 [-1, 1] to i16)
        for &sample in &self.samples {
            let s = sample.clamp(-1.0, 1.0);
            let pcm = (s * 32767.0) as i16;
            wav.extend_from_slice(&pcm.to_le_bytes());
        }
        
        wav
    }
    
    /// Get duration in seconds
    pub fn duration(&self) -> f32 {
        self.samples.len() as f32 / QWEN_TTS_SAMPLE_RATE as f32
    }
    
    /// Get number of samples
    pub fn num_samples(&self) -> usize {
        self.samples.len()
    }
}
