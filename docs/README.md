# Qwen3-TTS Rust 服务端开发文档

## 项目概述

本项目旨在将 Qwen3-TTS 的 C 语言推理引擎与 Rust 高性能服务端相结合，利用 Rust 的内存安全、并发模型和生态优势，构建一个现代化的 TTS 服务。

### 架构目标

1. **C 语言层**：专注于模型推理核心逻辑（已有实现）
2. **Rust 语言层**：负责 HTTP 服务、请求处理、线程管理、并发控制等
3. **FFI 桥接**：安全的 Rust-C 互操作接口

---

## 已完成任务

### Phase 1: FFI 绑定完善与基础语音合成功能 ✅

#### 1.1 错误处理系统
- ✅ 定义 `TtsErrorCode` 枚举（10 种错误类型）
- ✅ 实现 `TtsError` 结构体（code + message）
- ✅ 实现 `std::error::Error`, `Display`, `From` traits
- ✅ 线程局部错误检索：`qwen_tts_get_last_error()`

#### 1.2 Context 参数管理
- ✅ 实现 `TtsParams` 结构体：
  ```rust
  pub struct TtsParams {
      pub speaker_id: c_int,
      pub language_id: c_int,
      pub temperature: c_float,
      pub top_k: c_int,
      pub top_p: c_float,
      pub speed: c_float,
      pub volume: c_float,
  }
  ```
- ✅ Builder 模式：`.with_speaker()`, `.with_language()`, `.with_temperature()` 等
- ✅ 自动值域限制（clamping）
- ✅ `Default` 实现（Ryan, English, temp=0.7）

#### 1.3 安全 Context 封装 (`TtsContext`)
- ✅ RAII 资源管理（`Drop` trait 自动清理）
- ✅ 方法实现：
  - `new(model_dir)` - 初始化带错误处理
  - `set_params(params)` - 配置生成参数
  - `generate(text)` - 同步语音生成
  - `generate_with_params(text, params)` - 自定义参数生成
  - `as_raw_ptr()` - 高级用法逃生舱
- ✅ `Send` but not `Sync`（需 `Arc<Mutex<T>>` 共享访问）

#### 1.4 增强音频结果 (`TtsResult`)
- ✅ `to_wav()` - 转换为 WAV 字节（24kHz, 16-bit PCM）
- ✅ `to_wav_with_sample_rate(rate)` - 自定义采样率
- ✅ `duration()` - 获取音频时长（秒）
- ✅ `num_samples()` - 获取样本数

#### 1.5 扩展 FFI 函数声明
```rust
extern "C" {
    // 参数设置
    fn qwen_tts_set_params(ctx, speaker_id, language_id, temp, top_k, top_p) -> c_int;
    fn qwen_tts_set_synthesis_params(ctx, speed, volume, emotion) -> c_int;
    
    // 生成变体
    fn qwen_tts_generate_with_params(...) -> *mut c_float;
    fn qwen_tts_generate_stream_ex(ctx, text, audio_cb, progress_cb, userdata) -> c_int;
    
    // 内存管理
    fn qwen_tts_free_samples(samples);
    
    // 错误处理
    fn qwen_tts_get_last_error() -> *const c_char;
    fn qwen_tts_clear_error();
}
```

#### 1.6 回调类型
- ✅ `qwen_tts_audio_cb` - 流式音频回调
- ✅ `qwen_tts_progress_cb` - 进度报告回调（0.0 到 1.0）

#### 1.7 HTTP 服务端更新
- ✅ 应用状态使用 `Mutex<TtsContext>` 替代裸指针
- ✅ 增强请求模型（新增 `speed`, `volume` 参数）
- ✅ 错误转换函数：`tts_error_to_http_error()`
- ✅ 改进响应头（添加 `X-Audio-Duration`）
- ✅ 优化启动信息和 API 使用示例

---

### Phase 2 之前的历史完成项

#### 2.1 项目基础架构搭建

- ✅ 创建 `Cargo.toml` 项目配置文件
  - 依赖：`axum` (HTTP 框架), `tokio` (异步运行时), `serde` (序列化)
  - 构建脚本：`build.rs`
  
- ✅ 创建 `build.rs` 编译脚本
  - 自动编译 C 源码为静态库
  - 链接 `qwen_tts` 和 `server` 模块
  
- ✅ 创建 FFI 绑定模块 (`src/ffi/`)
  - `mod.rs`: FFI 模块入口和公共类型定义
  - `qwen_tts.rs`: TTS 引擎核心函数绑定
  - `server.rs`: 服务器生命周期管理绑定

#### 2.2 HTTP 服务端实现

- ✅ 创建 `src/main.rs` 主程序
  - Axum 路由配置
  - 异步请求处理
  
- ✅ 实现 API 端点
  | 端点 | 方法 | 功能 |
  |------|------|------|
  | `/v1/health` | GET | 健康检查 |
  | `/v1/speakers` | GET | 获取可用 speaker 列表 |
  | `/v1/tts` | POST | TTS 音频生成 (WAV) |
  | `/v1/audio/speech` | POST | OpenAI 兼容接口 |

#### 2.3 数据结构定义

- ✅ `TtsRequest`: 请求参数结构
  - `text`: 输入文本
  - `speaker`: 说话人名称（字符串）
  - `language`: 语言名称
  - `temperature`, `top_k`, `top_p`: 采样参数
  - `speed`, `volume`: 合成参数（Phase 1 新增）
  
- ✅ `TtsResult`: 音频结果结构
  - `samples`: f32 样本向量
  - `to_wav()`: WAV 编码方法
  - `duration()`: 时长计算

---

## 待完成任务

### 第一阶段：完善 FFI 绑定 (高优先级)

#### 1.1 Context 管理
- [ ] 实现 `qwen_tts_context_create` 的安全封装
- [ ] 实现 `qwen_tts_context_destroy` 的资源清理
- [ ] 添加 context 参数设置函数绑定：
  - `qwen_tts_set_device()` (CPU/GPU)
  - `qwen_tts_set_threads()` (线程数)
  - `qwen_tts_set_sample_rate()` (采样率)

#### 1.2 推理函数绑定
- [ ] 实现 `qwen_tts_generate` 的完整参数映射
- [ ] 添加流式推理接口 `qwen_tts_generate_stream`
- [ ] 处理音频输出缓冲区的内存管理

#### 1.3 错误处理
- [ ] 定义统一的错误码枚举
- [ ] 实现 C 错误码到 Rust Result 的转换
- [ ] 添加详细的错误日志记录

### 第二阶段：服务功能增强 (中优先级)

#### 2.1 并发与线程池
- [ ] 使用 tokio 实现异步并发请求处理
- [ ] 设计线程池管理多个 TTS context 实例
- [ ] 实现请求队列和限流机制
- [ ] 添加并发压力测试

#### 2.2 流式响应
- [ ] 实现 SSE (Server-Sent Events) 流式输出
- [ ] 支持 chunked transfer encoding
- [ ] 实现 WebSocket 实时音频流

#### 2.3 缓存优化
- [ ] 实现文本哈希缓存机制
- [ ] 添加 LRU 缓存策略
- [ ] 支持预加载常用 speaker 模型

### 第三阶段：C 代码迁移 (低优先级)

#### 3.1 逐步迁移计划
- [ ] 分析 C 语言端的预处理逻辑
- [ ] 用 Rust 重写文本规范化模块
- [ ] 用 Rust 重写音频后处理模块
- [ ] 性能对比测试

#### 3.2 新增功能
- [ ] 批量推理接口
- [ ] 多模型热切换
- [ ] 动态 speaker 嵌入加载
- [ ] 音频效果器 (混响、变调等)

### 第四阶段：生产化准备

#### 4.1 监控与日志
- [ ] 集成 tracing 日志系统
- [ ] 添加 Prometheus 指标导出
- [ ] 实现分布式追踪 (OpenTelemetry)

#### 4.2 部署与配置
- [ ] Docker 容器化配置
- [ ] Kubernetes Helm Chart
- [ ] 环境变量配置管理
- [ ] 热重载配置支持

#### 4.3 测试与文档
- [ ] 单元测试覆盖率达到 80%
- [ ] 集成测试套件
- [ ] API 文档 (OpenAPI/Swagger)
- [ ] 性能基准测试报告

---

## 技术栈说明

### Rust 依赖
```toml
[dependencies]
axum = "0.7"           # HTTP 框架
tokio = "1.35"         # 异步运行时
serde = "1.0"          # 序列化
serde_json = "1.0"     # JSON 处理
base64 = "0.21"        # Base64 编码
tracing = "0.1"        # 日志记录
thiserror = "1.0"      # 错误处理
```

### C 语言接口
```c
// 核心接口
qwen_tts_context_t* qwen_tts_context_create(const char* model_path);
void qwen_tts_context_destroy(qwen_tts_context_t* ctx);
int qwen_tts_generate(qwen_tts_context_t* ctx, const char* text, ...);

// 服务器接口
server_t* server_create(int port);
void server_start(server_t* srv);
void server_stop(server_t* srv);
```

---

## 构建与运行

### 前置条件
1. Rust 工具链 (rustc >= 1.75)
2. C 编译器 (gcc/clang)
3. CMake (如需编译依赖)

### 编译命令
```bash
# 调试版本
cargo build

# 发布版本
cargo build --release

# 运行服务
cargo run --release
```

### 环境变量
```bash
export QWEN_TTS_MODEL_PATH="/path/to/model"
export QWEN_TTS_THREADS="4"
export RUST_LOG="info"
```

---

## 性能目标

| 指标 | 目标值 | 测量方式 |
|------|--------|----------|
| P99 延迟 | < 500ms | 10 秒文本生成 |
| 吞吐量 | > 50 req/s | 并发请求 |
| 内存占用 | < 2GB | 单实例 |
| CPU 利用率 | > 80% | 多核负载 |

---

## 贡献指南

1. Fork 项目仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交变更 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 提交 Pull Request

### 代码规范
- 遵循 Rust 官方代码风格
- 所有公共函数必须有文档注释
- 错误处理使用 Result<T, E> 而非 panic
- 异步函数使用 tokio 运行时

---

## 参考资料

- [Qwen3-TTS C 语言实现](../qwen-tts-c/)
- [Rust FFI 指南](https://doc.rust-lang.org/nomicon/ffi.html)
- [Axum 框架文档](https://docs.rs/axum)
- [Tokio 异步编程](https://tokio.rs)

---

## 更新日志

### v0.1.0 (2025-01-XX)
- ✨ 初始项目架构搭建
- ✨ FFI 绑定基础框架
- ✨ HTTP 服务端点实现
- 📝 本文档创建

---

**维护者**: Qwen3-TTS Team  
**许可证**: Apache 2.0 / MIT
