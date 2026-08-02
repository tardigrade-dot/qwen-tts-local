use crate::bindings::*;
use std::ffi::CString;
use std::os::raw::c_char;
use std::ptr::NonNull;

#[derive(Clone, Copy)]
pub struct RawCtx(pub *mut qwen_tts_ctx);
unsafe impl Send for RawCtx {}
unsafe impl Sync for RawCtx {}

pub struct Context {
    raw: NonNull<qwen_tts_ctx>,
    is_clone: bool,
}

unsafe impl Send for Context {}
unsafe impl Sync for Context {}

impl Context {
    pub unsafe fn load(
        model_dir: &str,
        silent: bool,
        use_int8: bool,
        use_int4: bool,
    ) -> Option<Self> {
        let dir = CString::new(model_dir).ok()?;
        let ptr = qwen_tts_load_ex(
            dir.as_ptr() as *const c_char,
            silent as i32,
            use_int8 as i32,
            use_int4 as i32,
        );
        NonNull::new(ptr).map(|raw| Context {
            raw,
            is_clone: false,
        })
    }

    #[allow(dead_code)]
    pub unsafe fn clone_for_worker(&self) -> Option<Self> {
        let ptr = qwen_tts_clone_for_worker(self.raw.as_ptr());
        NonNull::new(ptr).map(|raw| Context {
            raw,
            is_clone: true,
        })
    }

    pub fn as_ptr(&self) -> *mut qwen_tts_ctx {
        self.raw.as_ptr()
    }

    pub unsafe fn cleanup_raw(ptr: *mut qwen_tts_ctx) {
        qwen_tts_unload(ptr);
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            if self.is_clone {
                qwen_tts_free_clone(self.as_ptr());
            } else {
                qwen_tts_unload(self.as_ptr());
            }
        }
    }
}

pub unsafe fn reset_context_state(ctx: *mut qwen_tts_ctx) {
    qwen_ctx_reset_state(ctx);
}

pub unsafe fn apply_params_to_context(ctx: *mut qwen_tts_ctx, params: &GenerationParams) {
    qwen_ctx_set_speaker_id(ctx, params.speaker_id);
    qwen_ctx_set_language_id(ctx, params.language_id);
    qwen_ctx_set_temperature(ctx, params.temperature);
    qwen_ctx_set_top_k(ctx, params.top_k);
    qwen_ctx_set_top_p(ctx, params.top_p);
    qwen_ctx_set_rep_penalty(ctx, params.rep_penalty);
    qwen_ctx_set_max_tokens(ctx, params.max_tokens);
    qwen_ctx_set_voice_design(ctx, params.voice_design as i32);
    qwen_ctx_set_voice_clone(ctx, params.voice_clone as i32);
    qwen_ctx_set_xvector_only(ctx, params.xvector_only as i32);
    qwen_ctx_set_cp_roughness(ctx, params.cp_roughness);

    if params.seed > 0 {
        qwen_ctx_set_seed(ctx, params.seed);
    } else {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        qwen_ctx_set_seed(ctx, ts);
    }

    qwen_ctx_set_instruct(ctx, std::ptr::null());
    if let Some(ref instr) = params.instruct {
        let c_str = CString::new(instr.as_str()).unwrap();
        qwen_ctx_set_instruct(ctx, c_str.as_ptr() as *const c_char);
    }

    qwen_ctx_set_ref_audio_path(ctx, std::ptr::null());
    if let Some(ref path) = params.ref_audio_path {
        let c_str = CString::new(path.as_str()).unwrap();
        qwen_ctx_set_ref_audio_path(ctx, c_str.as_ptr() as *const c_char);
    }

    qwen_ctx_set_ref_text(ctx, std::ptr::null());
    if let Some(ref text) = params.ref_text {
        let c_str = CString::new(text.as_str()).unwrap();
        qwen_ctx_set_ref_text(ctx, c_str.as_ptr() as *const c_char);
    }
}

pub unsafe fn apply_emotion(
    ctx: *mut qwen_tts_ctx,
    emotion_spec: &str,
    language: &str,
) -> (Option<f32>, Option<f32>) {
    let c_emotion = CString::new(emotion_spec).unwrap();
    let c_lang = CString::new(language).unwrap();
    let mut out_volume: f32 = 0.0;
    let mut out_rate: f32 = 0.0;

    let ret = qwen_tts_apply_emotion(
        ctx,
        c_emotion.as_ptr() as *const c_char,
        c_lang.as_ptr() as *const c_char,
        0.0, 0,
        0.0, 0,
        0.0, 0,
        &mut out_volume as *mut f32,
        &mut out_rate as *mut f32,
        0,
    );

    if ret == 0 {
        (Some(out_volume), Some(out_rate))
    } else {
        (None, None)
    }
}

pub unsafe fn language_id(name: &str) -> i32 {
    let c_name = CString::new(name).unwrap();
    qwen_tts_language_id(c_name.as_ptr() as *const c_char)
}

pub unsafe fn speaker_id(name: &str) -> i32 {
    let c_name = CString::new(name).unwrap();
    qwen_tts_speaker_id(c_name.as_ptr() as *const c_char)
}

#[derive(Clone)]
pub struct GenerationParams {
    pub text: String,
    pub speaker_id: i32,
    pub language_id: i32,
    pub temperature: f32,
    pub top_k: i32,
    pub top_p: f32,
    pub rep_penalty: f32,
    pub seed: u32,
    pub max_tokens: i32,
    pub instruct: Option<String>,
    pub voice_design: bool,
    pub voice_clone: bool,
    pub xvector_only: bool,
    pub ref_audio_path: Option<String>,
    pub ref_text: Option<String>,
    pub cp_roughness: f32,
    pub emotion: Option<String>,
    pub volume: Option<f32>,
    pub rate: Option<f32>,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            text: String::new(),
            speaker_id: 3061,
            language_id: 2050,
            temperature: 0.5,
            top_k: 50,
            top_p: 1.0,
            rep_penalty: 1.05,
            seed: 0,
            max_tokens: 8192,
            instruct: None,
            voice_design: false,
            voice_clone: false,
            xvector_only: false,
            ref_audio_path: None,
            ref_text: None,
            cp_roughness: 0.0,
            emotion: None,
            volume: None,
            rate: None,
        }
    }
}
