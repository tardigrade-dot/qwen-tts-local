# Qwen3-TTS Stream 首帧延迟波动分析

## 问题现象

在使用 stream 模式 (`/v1/tts/stream`) 时，首帧延迟 (Time-To-First-Audio, TTFA) 存在显著波动：
- **低延迟场景**: ~1-2 秒
- **高延迟场景**: ~7+ 秒

输入文本、模型参数完全一致，但延迟表现不稳定。

---

## C 代码架构分析

### 1. Streaming 路径概览

```
HTTP Server (qwen_tts_server.c)
    ↓ handle_tts_stream()
    ├─ send_chunked_header()  ← 立即发送 HTTP chunked 响应头
    ├─ qwen_tts_set_audio_callback()  ← 设置回调
    └─ qwen_tts_generate()
            ↓
        Talker 生成 codec tokens (逐帧)
            ↓
        Decoder Thread (decoder_thread_fn)
            ↓
        audio_cb (stream_http_callback) → write() 发送 chunk
```

**关键代码位置**:
- `qwen_tts_server.c:504-575` - `handle_tts_stream()`
- `qwen_tts_server.c:189-210` - `stream_http_callback()`
- `qwen_tts.c:1523-1792` - `qwen_tts_generate()` 中的 decoder thread 逻辑

---

## 潜在延迟来源分析

### 1. **Decoder Thread 启动时机** ⚠️

**代码位置**: `qwen_tts.c:1558-1577`

```c
if (!dt_no_overlap) {
    pthread_create(&dt_thread, NULL, decoder_thread_fn, &dt_state);
    // ... 调整 BLAS 线程数 ...
}
```

**问题分析**:
- Decoder thread 在 `qwen_tts_generate()` 被调用后**异步启动**
- 首个音频 chunk 需要等待 decoder thread 完成第一次 `pthread_cond_wait()` → 解码 → `audio_cb` 回调
- **竞争条件**: 如果 decoder thread 调度延迟（OS 调度器、CPU 争用），首帧会被推迟

**波动原因**:
- Linux CFS 调度器的非确定性
- 系统负载变化导致 thread 唤醒延迟
- NUMA 节点间内存访问延迟差异

---

### 2. **BLAS 线程动态调整** ⚠️⚠️

**代码位置**: `qwen_tts.c:1569-1576`

```c
int nt = qwen_get_threads();
int gen_blas = nt > 1 ? nt - 1 : 1;
{ const char *e = getenv("QWEN_BLAS_GEN_THREADS");
  if (e && atoi(e) > 0) gen_blas = atoi(e); }
qwen_blas_set_threads(gen_blas);
```

**问题分析**:
- 在 decoder thread 启动**之后**立即调用 `qwen_blas_set_threads()`
- OpenBLAS 内部可能触发**线程池重建**（尤其是首次调用或线程数变化时）
- OpenBLAS 线程创建/销毁是**阻塞操作**，可能与 decoder thread 争抢 CPU

**波动原因**:
- OpenBLAS 线程池状态不确定（之前是否有其他请求用过 BLAS）
- `gen_blas` 计算依赖 `qwen_get_threads()`，而该值可能受前序请求影响
- 如果 `QWEN_BLAS_GEN_THREADS` 未设置，默认 `nt-1` 在不同负载下可能不同

---

### 3. **Thread Pool Spin/Park 行为** ⚠️

**代码位置**: `qwen_tts_thread.c:254-270`

```c
int budget = qwen_pool_spin();  // 默认 4096 次
while (budget-- > 0 &&
       atomic_load_explicit(&P.generation, memory_order_acquire) == seen &&
       !P.stop)
    qwen_cpu_relax();

if (atomic_load_explicit(&P.generation, memory_order_acquire) == seen) {
    // 自旋耗尽后 park 到 condvar
    pthread_mutex_lock(&P.mtx);
    P.sleeping++;
    while (...) pthread_cond_wait(&P.wake, &P.mtx);
    ...
}
```

**问题分析**:
- Worker threads 在空闲时先**自旋** (最多 4096 次 `pause`/`yield` 指令)，然后 park
- 如果自旋期间没有新任务，thread 进入 condvar 睡眠
- **下次任务到达时**: 
  - 如果 thread 还在自旋 → 快速唤醒（微秒级）
  - 如果 thread 已 park → 需要 futex syscall（毫秒级）

**波动原因**:
- 请求间隔时间不确定：
  - 短间隔 (< 几 ms): worker 仍在自旋 → 低延迟
  - 长间隔 (> 几 ms): worker 已 park → 高延迟（futex 唤醒开销）
- 系统 futex 实现差异（内核版本、配置）

---

### 4. **Speech Decoder 内部线程池** ⚠️

**代码位置**: `qwen_tts_kernels.c:3661-3700`

```c
static int sd_pool_threads(void) {
    static int cfg = -1;
    if (cfg < 0) { const char *e = getenv("QWEN_SD_THREADS"); cfg = e ? atoi(e) : 0; }
    return cfg > 0 ? cfg : qwen_get_threads();
}
```

**问题分析**:
- Speech decoder 有自己独立的线程池 (`sdp_threads[]`)
- 首次调用时**惰性创建** worker threads (`sdp_started` 标志)
- 线程创建是阻塞操作，可能发生在**首个 frame 解码时**

**波动原因**:
- 如果是第一个使用 streaming 的请求 → 需要创建 decoder 线程池 → 额外延迟
- 后续请求复用已有线程池 → 延迟较低
- `QWEN_SD_THREADS` 未设置时，依赖 `qwen_get_threads()`，可能受前序请求影响

---

### 5. **Chunk Frames 大小影响**

**代码位置**: `qwen_tts_server.c:531-534`

```c
int chunk_frames = (int)json_extract_number(body, "chunk_frames", 10);
if (chunk_frames < 2)   chunk_frames = 2;
if (chunk_frames > 250) chunk_frames = 250;
ctx->stream_chunk_frames = chunk_frames;
```

**问题分析**:
- 默认 `chunk_frames = 10` (0.8s 音频)
- Decoder 需要累积至少一个 chunk 才能触发 `audio_cb`
- **首帧延迟 = 生成 10 frames 的时间 + 解码时间 + 回调开销**

**波动原因**:
- Talker 生成速度受 CPU 负载、BLAS 状态影响
- 如果生成速度慢，累积 10 frames 的时间会显著增加

---

### 6. **OpenBLAS 线程数竞争**

**代码位置**: `qwen_tts_kernels.c:70-76`

```c
extern void openblas_set_num_threads(int) __attribute__((weak));

void qwen_blas_set_threads(int n) {
    if (getenv("OPENBLAS_NUM_THREADS")) return;
    if (openblas_set_num_threads) openblas_set_num_threads(n > 0 ? n : 1);
}
```

**问题分析**:
- OpenBLAS 线程数是**全局状态**
- 多个并发请求可能互相覆盖线程数设置
- OpenBLAS 内部线程池可能对线程数变化做出激进反应（重建 pool）

**波动原因**:
- 如果有其他请求同时修改 BLAS 线程数 → 不可预测的性能抖动
- OpenBLAS 版本差异（某些版本对 `set_num_threads` 更敏感）

---

### 7. **Tokenizer 缓存状态**

**代码位置**: `qwen_tts.c:948-953`

```c
qwen_tokenizer_t *tok = (qwen_tokenizer_t *)ctx->cached_tokenizer;
if (!tok) {
    tok = qwen_tokenizer_encode(tok, instruct_tmpl, &instruct_token_len);
    if (tok) ctx->cached_tokenizer = tok;
}
```

**问题分析**:
- Tokenizer 首次加载需要读取文件、构建 BPE 表
- 如果 `cached_tokenizer == NULL` → 同步加载 → 阻塞首帧

**波动原因**:
- 冷启动 vs 热缓存
- 文件系统缓存状态（模型文件是否已在 page cache）

---

## 综合延迟路径

```
请求到达
    ↓
[可选] Tokenizer 加载 (冷启动时 ~10-100ms)
    ↓
send_chunked_header() (立即，<1ms)
    ↓
qwen_tts_generate() 开始
    ↓
├─ Prefill (Talker 前置计算) [串行]
│   └─ 受 BLAS 线程数影响
    ↓
├─ pthread_create(decoder_thread) [异步]
│   └─ OS 调度延迟 (0.1-5ms)
    ↓
├─ qwen_blas_set_threads(gen_blas) [串行]
│   └─ OpenBLAS 线程池调整 (0-50ms) ⚠️
    ↓
├─ Frame 0 生成 (Talker)
│   └─ 受 CPU 负载、BLAS 状态影响
    ↓
├─ Decoder Thread 首次 wake up
│   ├─ 如果已 park: futex 唤醒 (~0.5-2ms) ⚠️
│   └─ 如果自旋中: 快速响应 (<0.1ms)
    ↓
├─ [可选] Speech Decoder 线程池创建 (首次时 ~5-20ms) ⚠️
    ↓
├─ 累积 chunk_frames (默认 10 frames)
│   └─ 10 × (Talker 生成 + 解码) 时间
    ↓
└─ stream_http_callback() → write() [首帧发出]
```

**总延迟波动范围**: 
- **最佳情况**: 1-2s (所有线程已预热，BLAS 状态稳定)
- **最坏情况**: 7+s (冷启动 + BLAS 调整 + futex 唤醒 + decoder 线程池创建)

---

## 诊断建议

### 1. 环境变量调试

```bash
# 固定 BLAS 线程数，避免动态调整
export QWEN_BLAS_GEN_THREADS=2

# 禁用 decoder thread 自旋，强制 park (减少 CPU 占用，但可能增加延迟)
export QWEN_POOL_SPIN=0

# 固定 Speech Decoder 线程数
export QWEN_SD_THREADS=2

# 同步解码 (禁用 overlap)，排除 thread 调度因素
export QWEN_NO_OVERLAP=1

# 测试单线程模式
export OPENBLAS_NUM_THREADS=1
```

### 2. 日志增强

在以下位置添加时间戳日志：
- `qwen_tts_generate()` 入口
- `pthread_create()` 前后
- `qwen_blas_set_threads()` 前后
- Decoder thread 首次 `pthread_cond_wait()` 唤醒
- 首个 `audio_cb` 回调

### 3. perf/strace 分析

```bash
# 追踪 futex syscall (thread park/wake)
sudo perf record -e syscalls:sys_enter_futex -g ./qwen_tts --serve ...

# 追踪 OpenBLAS 线程活动
strace -f -e trace=schedule ./qwen_tts ...
```

---

## 潜在优化方向

### 1. **预热线程池**
在服务器启动时预先创建所有线程（decoder、BLAS、speech decoder），避免运行时惰性创建。

### 2. **固定 BLAS 配置**
启动时一次性设置 `qwen_blas_set_threads()`，不在每请求中动态调整。

### 3. **减少 chunk_frames 默认值**
降低首帧等待时间（如从 10 降到 4 frames），但可能降低吞吐量。

### 4. **优先调度 Decoder Thread**
使用 `pthread_setschedparam()` 设置更高优先级，减少 OS 调度延迟。

### 5. **避免 BLAS 线程数竞争**
为每个并发请求使用独立的 BLAS 上下文（如果 OpenBLAS 支持）。

---

## 参考文档

- C 代码: `/workspace/qwen3-tts-c/qwen_tts.c`, `qwen_tts_server.c`, `qwen_tts_thread.c`, `qwen_tts_kernels.c`
- 相关 PR: #17 (batching server, thread pool spin optimization)
- 博客: `blog/making-qwen3-tts-fast-on-every-cpu.md`

---

**分析日期**: 2024
**分析工具**: C 源代码审查
