# FFI 绑定实现指南

本文档详细说明如何实现 Rust 与 C 语言 Qwen3-TTS 引擎的安全互操作。

## 目录

1. [FFI 基础概念](#ffi-基础概念)
2. [类型映射](#类型映射)
3. [内存安全实践](#内存安全实践)
4. [错误处理模式](#错误处理模式)
5. [示例代码](#示例代码)

---

## FFI 基础概念

### extern "C" 块

所有 C 函数声明必须放在 `extern "C"` 块中：

```rust
#[repr(C)]
pub struct QwenTTSContext {
    // 不透明指针，实际结构在 C 端定义
    _private: [u8; 0],
}

extern "C" {
    pub fn qwen_tts_context_create(model_path: *const c_char) -> *mut QwenTTSContext;
    pub fn qwen_tts_context_destroy(ctx: *mut QwenTTSContext);
    pub fn qwen_tts_generate(
        ctx: *mut QwenTTSContext,
        text: *const c_char,
        speaker_id: i32,
        speed: f32,
        output_buffer: *mut c_void,
        output_size: *mut usize,
    ) -> i32;
}
```

### 不透明指针模式

C 结构体在 Rust 中应作为不透明类型处理：

```rust
// 正确做法：零大小类型
#[repr(C)]
pub struct QwenTTSContext {
    _private: [u8; 0],
}

// 错误做法：尝试复制 C 结构体布局
// #[repr(C)]
// pub struct QwenTTSContext {
//     field1: i32,  // ❌ 可能与 C 端不一致
//     field2: *mut c_void,
// }
```

---

## 类型映射

### 基本类型对应表

| C 类型 | Rust 类型 | 说明 |
|--------|-----------|------|
| `int` | `i32` | 有符号整数 |
| `unsigned int` | `u32` | 无符号整数 |
| `float` | `f32` | 单精度浮点 |
| `double` | `f64` | 双精度浮点 |
| `char*` | `*mut c_char` | C 字符串指针 |
| `void*` | `*mut c_void` | 通用指针 |
| `size_t` | `usize` | 平台相关大小 |

### 字符串转换

```rust
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// Rust String -> C char*
fn rust_to_c_string(s: &str) -> Result<*mut c_char, std::ffi::NulError> {
    let c_string = CString::new(s)?;
    Ok(c_string.into_raw())
}

// C char* -> Rust String (安全版本)
unsafe fn c_to_rust_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string())
}

// C char* -> Rust String (释放内存版本)
unsafe fn c_to_rust_string_free(ptr: *mut c_char) -> Option<String> {
    let result = c_to_rust_string(ptr as *const c_char);
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
    result
}
```

### 缓冲区处理

```rust
use std::slice;

// C 输出缓冲区 -> Rust Vec<u8>
unsafe fn buffer_to_vec(data: *mut c_void, size: usize) -> Vec<u8> {
    if data.is_null() || size == 0 {
        return Vec::new();
    }
    let slice = slice::from_raw_parts(data as *const u8, size);
    slice.to_vec()
}

// Rust Vec<u8> -> C 缓冲区 (需要手动管理内存)
fn vec_to_c_buffer(data: Vec<u8>) -> (*mut c_void, usize) {
    let len = data.len();
    let ptr = Box::into_raw(data.into_boxed_slice()) as *mut c_void;
    (ptr, len)
}
```

---

## 内存安全实践

### RAII 封装

使用 `Drop` trait 自动清理 C 资源：

```rust
use std::ptr::NonNull;
use std::ffi::CString;
use std::os::raw::c_char;

pub struct TTSContext {
    ptr: NonNull<QwenTTSContext>,
}

impl TTSContext {
    pub fn new(model_path: &str) -> Result<Self, TTSError> {
        let c_path = CString::new(model_path)
            .map_err(|_| TTSError::InvalidPath)?;
        
        unsafe {
            let ptr = qwen_tts_context_create(c_path.as_ptr());
            if ptr.is_null() {
                return Err(TTSError::CreationFailed);
            }
            Ok(TTSContext {
                ptr: NonNull::new_unchecked(ptr),
            })
        }
    }
    
    pub fn generate(&self, text: &str, speaker_id: i32) -> Result<Vec<u8>, TTSError> {
        let c_text = CString::new(text)
            .map_err(|_| TTSError::InvalidText)?;
        
        let mut output_size: usize = 0;
        let mut output_buffer: *mut c_void = std::ptr::null_mut();
        
        unsafe {
            let ret = qwen_tts_generate(
                self.ptr.as_mut(),
                c_text.as_ptr(),
                speaker_id,
                1.0,
                &mut output_buffer as *mut _ as *mut c_void,
                &mut output_size,
            );
            
            if ret != 0 {
                return Err(TTSError::GenerationFailed(ret));
            }
            
            if output_buffer.is_null() {
                return Err(TTSError::EmptyOutput);
            }
            
            // 复制数据，然后释放 C 端分配的内存
            let result = buffer_to_vec(output_buffer, output_size);
            qwen_tts_free_buffer(output_buffer); // 假设有这个函数
            Ok(result)
        }
    }
}

impl Drop for TTSContext {
    fn drop(&mut self) {
        unsafe {
            qwen_tts_context_destroy(self.ptr.as_mut());
        }
    }
}

// 确保不跨线程移动 (如果 C 库不是线程安全的)
impl !Send for TTSContext {}
impl !Sync for TTSContext {}
```

### 生命周期标注

```rust
// 当 Rust 借用传递给 C 时，确保生命周期足够长
pub fn process_with_c<'a>(
    context: &TTSContext,
    text: &'a str,
    callback: impl FnOnce(&'a [u8]) + 'a,
) -> Result<(), TTSError> {
    let audio_data = context.generate(text, 0)?;
    callback(&audio_data);
    Ok(())
}
```

---

## 错误处理模式

### 错误码枚举

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TTSError {
    #[error("无效的文件路径")]
    InvalidPath,
    
    #[error("上下文创建失败")]
    CreationFailed,
    
    #[error("生成失败，错误码：{0}")]
    GenerationFailed(i32),
    
    #[error("输出为空")]
    EmptyOutput,
    
    #[error("无效的文本输入")]
    InvalidText,
    
    #[error("FFI 调用失败：{0}")]
    FfiError(String),
}

// C 错误码映射
pub fn map_c_error(code: i32) -> TTSError {
    match code {
        0 => return TTSError::FfiError("未知错误".to_string()),
        -1 => TTSError::CreationFailed,
        -2 => TTSError::GenerationFailed(code),
        -3 => TTSError::EmptyOutput,
        _ => TTSError::GenerationFailed(code),
    }
}
```

### Result 包装器

```rust
pub type TTSResult<T> = Result<T, TTSError>;

// 安全的 FFI 调用宏
macro_rules! ffi_call {
    ($func:ident ($($arg:expr),*)) => {
        unsafe {
            let ret = $func($($arg),*);
            if ret != 0 {
                return Err(map_c_error(ret));
            }
            ret
        }
    };
}

// 使用示例
pub fn set_threads(context: &TTSContext, threads: i32) -> TTSResult<()> {
    unsafe {
        let ret = qwen_tts_set_threads(context.ptr.as_mut(), threads);
        if ret != 0 {
            return Err(map_c_error(ret));
        }
    }
    Ok(())
}
```

---

## 示例代码

### 完整的 TTS 请求处理

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

// 线程安全的上下文池
pub struct ContextPool {
    contexts: Arc<Mutex<Vec<TTSContext>>>,
}

impl ContextPool {
    pub fn new(model_path: &str, pool_size: usize) -> Result<Self, TTSError> {
        let mut contexts = Vec::with_capacity(pool_size);
        for _ in 0..pool_size {
            contexts.push(TTSContext::new(model_path)?);
        }
        Ok(ContextPool {
            contexts: Arc::new(Mutex::new(contexts)),
        })
    }
    
    pub async fn generate(
        &self,
        text: String,
        speaker_id: i32,
    ) -> TTSResult<Vec<u8>> {
        // 从池中获取一个 context
        let context = {
            let mut guard = self.contexts.lock().await;
            guard.pop().ok_or(TTSError::CreationFailed)?
        };
        
        // 执行推理
        let result = context.generate(&text, speaker_id);
        
        // 归还 context 到池中
        {
            let mut guard = self.contexts.lock().await;
            guard.push(context);
        }
        
        result
    }
}

// Axum handler
use axum::{
    extract::State,
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct TTSRequest {
    text: String,
    speaker_id: i32,
    speed: Option<f32>,
}

#[derive(Serialize)]
pub struct TTSResponse {
    audio_data: String,
    duration: f32,
}

pub async fn tts_handler(
    State(pool): State<Arc<ContextPool>>,
    Json(req): Json<TTSRequest>,
) -> Result<Json<TTSResponse>, StatusCode> {
    let audio_data = pool
        .generate(req.text, req.speaker_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let duration = audio_data.len() as f32 / 24000.0 / 2.0; // 假设 24kHz, 16bit
    
    Ok(Json(TTSResponse {
        audio_data: base64::encode(&audio_data),
        duration,
    }))
}
```

### 流式响应实现

```rust
use axum::{
    response::sse::{Event, Sse},
    streaming::Stream,
};
use futures_util::stream::StreamExt;
use std::time::Duration;
use tokio::time::interval;

pub async fn tts_stream_handler(
    State(pool): State<Arc<ContextPool>>,
    Json(req): Json<TTSRequest>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let stream = async_stream::stream! {
        let audio_data = match pool.generate(req.text, req.speaker_id).await {
            Ok(data) => data,
            Err(_) => return,
        };
        
        // 分块发送音频数据
        const CHUNK_SIZE: usize = 4096;
        for chunk in audio_data.chunks(CHUNK_SIZE) {
            yield Ok(Event::default().data(base64::encode(chunk)));
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(1))
            .text("ping"),
    )
}
```

---

## 调试技巧

### 启用详细日志

```rust
use tracing::{info, warn, error};

pub fn debug_ffi_call(func_name: &str, result: i32) {
    info!("FFI call: {} returned {}", func_name, result);
    if result != 0 {
        error!("FFI error in {}: code={}", func_name, result);
    }
}
```

### 内存泄漏检测

```bash
# 使用 valgrind 检测
valgrind --leak-check=full ./target/release/qwen-tts-server

# 使用 AddressSanitizer
export RUSTFLAGS="-Z sanitizer=address"
cargo run
```

### FFI 边界断言

```rust
#[cfg(debug_assertions)]
fn validate_ffi_boundaries() {
    assert_eq!(std::mem::size_of::<i32>(), 4);
    assert_eq!(std::mem::align_of::<f64>(), 8);
    assert_eq!(std::mem::size_of::<*const c_void>(), std::mem::size_of::<usize>());
}
```

---

## 性能优化建议

1. **减少 FFI 调用次数**: 批量处理数据而非逐元素调用
2. **避免不必要的字符串转换**: 缓存 CStr 或使用字节切片
3. **零拷贝传输**: 使用 mmap 或共享内存传递大数据
4. **异步非阻塞**: 将阻塞的 FFI 调用放入 tokio::task::spawn_blocking

```rust
// 正确的异步 FFI 调用
pub async fn generate_async(context: Arc<TTSContext>, text: String) -> TTSResult<Vec<u8>> {
    tokio::task::spawn_blocking(move || {
        context.generate(&text, 0)
    })
    .await
    .map_err(|_| TTSError::FfiError("任务取消".to_string()))?
}
```

---

## 常见问题

### Q: 如何处理 C 端的回调函数？

A: 使用 `extern "C"` 定义回调签名，并通过 `Box::into_raw` 传递 Rust 闭包：

```rust
extern "C" fn progress_callback(progress: f32, user_data: *mut c_void) {
    unsafe {
        let callback = &*(user_data as *const Box<dyn Fn(f32)>);
        callback(progress);
    }
}

// 使用时
let callback = Box::new(|p: f32| println!("Progress: {}%", p * 100.0));
let ptr = Box::into_raw(callback);
qwen_tts_set_callback(ctx.ptr.as_mut(), Some(progress_callback), ptr as *mut c_void);
```

### Q: 如何保证多线程安全？

A: 
1. 如果 C 库是线程安全的，为 `TTSContext` 实现 `Send + Sync`
2. 否则，使用 `Mutex` 或每个线程独立的 context
3. 考虑使用 `thread_local!` 存储每线程 context

---

**最后更新**: 2025-01-XX  
**作者**: Qwen3-TTS Team
