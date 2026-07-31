//! FFI bindings for qwen_tts_server.h - HTTP server functionality

use std::ffi::{c_char, c_int};
use crate::ffi::qwen_tts::qwen_tts_ctx_t;

extern "C" {
    /// Start a simple HTTP server (single-threaded, blocking)
    /// Returns 0 on clean shutdown, -1 on error
    pub fn qwen_tts_serve(ctx: *mut qwen_tts_ctx_t, port: c_int) -> c_int;
    
    /// Start HTTP server with worker pool for concurrent synthesis
    /// n_workers <= 1: single-threaded inline accept loop
    /// n_workers >= 2: acceptor thread + worker pool
    /// Returns 0 on clean shutdown, -1 on error
    pub fn qwen_tts_serve_ex(ctx: *mut qwen_tts_ctx_t, port: c_int, n_workers: c_int) -> c_int;
    
    /// Start vLLM-style request-batching server
    /// max_batch >= 2 for batching, scheduler thread owns ctx
    /// Returns 0 on clean shutdown, -1 on error
    pub fn qwen_tts_serve_batched(ctx: *mut qwen_tts_ctx_t, port: c_int, max_batch: c_int) -> c_int;
}
