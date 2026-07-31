# 开发路线图 (Roadmap)

本文档定义 Qwen3-TTS Rust 服务端的开发阶段、里程碑和优先级。

## 总体时间线

```
Phase 1 (2-4 周) → Phase 2 (3-5 周) → Phase 3 (4-6 周) → Phase 4 (持续)
    FFI 完善           功能增强          C 代码迁移        生产化
```

---

## Phase 1: 完善 FFI 绑定 (高优先级)

**目标**: 建立安全、完整的 Rust-C 互操作层

**预计时间**: 2-4 周

### Sprint 1.1: Context 管理 (第 1 周)

#### 任务清单
- [ ] 实现 `QwenTTSContext` RAII 封装
  - `new()` 构造函数
  - `Drop` trait 实现
  - `Send/Sync` 安全性分析
  
- [ ] Context 参数设置 API
  ```rust
  pub fn set_device(&self, device: Device) -> TTSResult<()>;
  pub fn set_threads(&self, threads: i32) -> TTSResult<()>;
  pub fn set_sample_rate(&self, rate: i32) -> TTSResult<()>;
  pub fn set_voice(&self, voice_id: i32) -> TTSResult<()>;
  ```

- [ ] 单元测试覆盖
  - Context 创建/销毁测试
  - 参数边界值测试
  - 多线程访问测试

#### 验收标准
- ✅ 所有 context 操作通过 RAII 自动管理
- ✅ 无内存泄漏 (valgrind 验证)
- ✅ 单元测试覆盖率 > 90%

### Sprint 1.2: 推理函数绑定 (第 2 周)

#### 任务清单
- [ ] 同步推理接口
  ```rust
  pub fn generate(&self, request: &TTSRequest) -> TTSResult<AudioBuffer>;
  ```

- [ ] 流式推理接口
  ```rust
  pub fn generate_stream(
      &self,
      text: &str,
      callback: impl FnMut(AudioChunk),
  ) -> TTSResult<()>;
  ```

- [ ] 音频缓冲区管理
  - 输出格式转换 (PCM/WAV/MP3)
  - 采样率重采样支持
  - 多通道音频处理

- [ ] 性能基准测试
  - 单次推理延迟测量
  - 吞吐量测试
  - 内存占用分析

#### 验收标准
- ✅ 支持同步和流式两种模式
- ✅ 音频质量与 C 端原生实现一致
- ✅ 性能开销 < 5% (相比纯 C 实现)

### Sprint 1.3: 错误处理与日志 (第 3 周)

#### 任务清单
- [ ] 统一错误类型设计
  ```rust
  #[derive(Error, Debug)]
  pub enum TTSError {
      #[error("模型加载失败：{0}")]
      ModelLoadError(String),
      #[error("推理失败：{0}")]
      InferenceError(String),
      #[error("资源不足：{0}")]
      ResourceExhausted(String),
      // ... 更多错误类型
  }
  ```

- [ ] 错误码映射表
  | C 错误码 | Rust 错误 | 说明 |
  |----------|-----------|------|
  | -1 | `ModelLoadError` | 模型文件问题 |
  | -2 | `InferenceError` | 推理过程错误 |
  | -3 | `ResourceExhausted` | 内存不足 |

- [ ] 集成 tracing 日志系统
  ```rust
  use tracing::{info, warn, error, debug};
  
  pub fn generate(&self, text: &str) -> TTSResult<Vec<u8>> {
      debug!("开始 TTS 生成，文本长度：{}", text.len());
      // ... 实现
      info!("TTS 生成完成，音频时长：{}ms", duration);
  }
  ```

- [ ] 结构化日志输出
  - JSON 格式日志
  - 请求 ID 追踪
  - 性能指标记录

#### 验收标准
- ✅ 所有 FFI 调用都有错误处理
- ✅ 错误信息包含足够的调试上下文
- ✅ 日志级别可配置 (RUST_LOG)

### Sprint 1.4: 文档与示例 (第 4 周)

#### 任务清单
- [ ] API 文档生成 (rustdoc)
- [ ] 使用示例代码
  - 基本用法示例
  - 高级配置示例
  - 并发使用示例
  
- [ ] FFI 安全指南编写
- [ ] 常见问题 FAQ

#### 交付物
- 📄 `docs/FFI_GUIDE.md` (已完成)
- 📄 `examples/basic_usage.rs`
- 📄 `examples/concurrent_server.rs`
- 🌐 在线 API 文档

---

## Phase 2: 服务功能增强 (中优先级)

**目标**: 构建高性能、可扩展的 HTTP 服务

**预计时间**: 3-5 周

### Sprint 2.1: 并发架构 (第 1-2 周)

#### 任务清单
- [ ] Context 池实现
  ```rust
  pub struct ContextPool {
      contexts: Vec<Arc<Mutex<TTSContext>>>,
      strategy: LoadBalanceStrategy,
  }
  
  pub enum LoadBalanceStrategy {
      RoundRobin,
      LeastLoaded,
      Random,
  }
  ```

- [ ] 异步请求处理
  ```rust
  pub async fn handle_tts_request(
      pool: Arc<ContextPool>,
      req: TTSRequest,
  ) -> TTSResult<AudioResponse>;
  ```

- [ ] 限流与背压
  - Token bucket 限流算法
  - 请求队列管理
  - 超时控制

- [ ] 压力测试
  - wrk/ab 工具配置
  - 并发用户数测试
  - 稳定性测试 (72 小时+)

#### 验收标准
- ✅ 支持 100+ 并发请求
- ✅ P99 延迟 < 500ms
- ✅ 无请求丢失或死锁

### Sprint 2.2: 流式传输 (第 3 周)

#### 任务清单
- [ ] SSE (Server-Sent Events) 实现
  ```rust
  pub async fn tts_stream(
      State(pool): State<Arc<ContextPool>>,
      Json(req): Json<TTSRequest>,
  ) -> Sse<impl Stream<Item = Result<Event, Infallible>>>;
  ```

- [ ] WebSocket 支持
  - 双向音频流
  - 实时控制命令
  - 心跳保活

- [ ] HTTP Chunked Transfer
  - 渐进式音频下载
  - Range 请求支持

#### 验收标准
- ✅ 首包延迟 < 100ms
- ✅ 流式传输无卡顿
- ✅ 支持断点续传

### Sprint 2.3: 缓存优化 (第 4 周)

#### 任务清单
- [ ] 文本哈希缓存
  ```rust
  use lru::LruCache;
  
  pub struct TTSCache {
      cache: LruCache<u64, AudioBuffer>,
      hit_rate: AtomicUsize,
      miss_rate: AtomicUsize,
  }
  ```

- [ ] Speaker 模型预加载
  - 热点 speaker 常驻内存
  - 懒加载冷数据
  - 动态卸载策略

- [ ] 缓存一致性
  - TTL 过期策略
  - 手动失效接口
  - 分布式缓存同步 (可选)

#### 验收标准
- ✅ 缓存命中率 > 60% (典型场景)
- ✅ 缓存查询延迟 < 1ms
- ✅ 内存占用可控

### Sprint 2.4: API 扩展 (第 5 周)

#### 新增端点
- [ ] `POST /v1/batch/tts` - 批量推理
- [ ] `GET /v1/models` - 模型列表
- [ ] `POST /v1/models/{id}/unload` - 卸载模型
- [ ] `GET /v1/metrics` - Prometheus 指标

#### OpenAI 兼容性
- [ ] 完全兼容 `/v1/audio/speech`
- [ ] 支持所有 `response_format` 选项
- [ ] 实现 `stream_options`

---

## Phase 3: C 代码迁移 (低优先级)

**目标**: 逐步将 C 端逻辑迁移到 Rust，提升可维护性

**预计时间**: 4-6 周

### Sprint 3.1: 文本预处理 (第 1-2 周)

#### 迁移模块
- [ ] 文本规范化 (Text Normalization)
  ```rust
  pub fn normalize_text(text: &str) -> String {
      // 数字转文字
      // 特殊符号处理
      // 多语言支持
  }
  ```

- [ ] phonemizer (音素转换)
  - 英文 phonemizer
  - 中文拼音转换
  - 多语言混合处理

#### 验证方法
- 对比 C/Rust 输出一致性
- 性能基准测试
- 边界 case 测试

### Sprint 3.2: 音频后处理 (第 3-4 周)

#### 迁移模块
- [ ] WAV 编码器
  ```rust
  pub fn encode_wav(pcm: &[f32], sample_rate: i32) -> Vec<u8>;
  ```

- [ ] MP3 编码器 (可选)
  - lame-rs 集成
  - 比特率控制

- [ ] 音频效果器
  - 音量归一化
  - 淡入淡出
  - 降噪处理

### Sprint 3.3: 性能优化 (第 5-6 周)

#### 优化方向
- [ ] SIMD 加速
  ```rust
  use std::arch::x86_64::*;
  
  #[target_feature(enable = "avx2")]
  pub unsafe fn normalize_audio_avx2(audio: &mut [f32]);
  ```

- [ ] 多线程并行
  - Rayon 数据并行
  - 流水线处理

- [ ] 内存优化
  - 对象池复用
  - 零拷贝序列化

#### 验收标准
- ✅ Rust 实现性能 >= C 实现
- ✅ 代码可读性提升
- ✅ 测试覆盖率 > 85%

---

## Phase 4: 生产化准备 (持续)

**目标**: 达到生产环境部署标准

### 监控与可观测性

#### 指标收集
- [ ] Prometheus 指标
  ```rust
  use prometheus::{Registry, Counter, Histogram};
  
  pub struct Metrics {
      requests_total: Counter,
      request_duration: Histogram,
      audio_duration: Histogram,
      cache_hit_ratio: Gauge,
  }
  ```

- [ ] 分布式追踪
  - OpenTelemetry 集成
  - Jaeger/Zipkin 后端
  - 跨服务追踪

#### 日志系统
- [ ] 结构化日志 (JSON)
- [ ] 日志聚合 (ELK/Loki)
- [ ] 告警规则配置

### 部署与运维

#### 容器化
- [ ] Dockerfile 优化
  ```dockerfile
  FROM rust:1.75 AS builder
  # ... 构建步骤
  
  FROM gcr.io/distroless/base-debian12
  COPY --from=builder /app/qwen-tts-server /usr/bin/
  CMD ["qwen-tts-server"]
  ```

- [ ] Kubernetes 配置
  - Deployment YAML
  - Service/Ingress
  - HPA 自动扩缩容

- [ ] Helm Chart
  - values.yaml 配置
  - 多环境支持 (dev/staging/prod)

#### 配置管理
- [ ] 环境变量支持
- [ ] 配置文件热重载
- [ ] Secrets 管理 (Vault/K8s Secrets)

### 测试体系

#### 单元测试
- [ ] 核心逻辑单元测试
- [ ] Mock C 函数测试
- [ ] 属性测试 (proptest)

#### 集成测试
- [ ] HTTP API 集成测试
- [ ] 端到端测试
- [ ] 兼容性测试 (多平台)

#### 性能测试
- [ ] 基准测试套件
- [ ] 负载测试脚本
- [ ] 长期稳定性测试

### 安全加固

#### 安全检查
- [ ] Cargo-audit 依赖审计
- [ ] Clippy lint 修复
- [ ] 模糊测试 (cargo-fuzz)

#### 权限控制
- [ ] API Key 认证
- [ ] JWT Token 验证
- [ ] IP 白名单

---

## 里程碑检查点

### M1: FFI 完成 (Week 4)
- ✅ 所有 C 函数有安全 Rust 封装
- ✅ 错误处理完善
- ✅ 基础文档完成

### M2: 服务可用 (Week 8)
- ✅ HTTP API 正常工作
- ✅ 支持 50+ 并发
- ✅ 缓存机制生效

### M3: 性能达标 (Week 12)
- ✅ P99 延迟 < 500ms
- ✅ 吞吐量 > 50 req/s
- ✅ 内存占用 < 2GB

### M4: 生产就绪 (Week 16+)
- ✅ 监控告警完善
- ✅ 自动化测试覆盖
- ✅ 部署文档齐全

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| C 库线程不安全 | 高 | 中 | 每线程独立 context，加 Mutex |
| FFI 内存泄漏 | 高 | 低 | Valgrind 定期检测，RAII 封装 |
| 性能不达标 | 中 | 中 | 早期基准测试，SIMD 优化 |
| 依赖兼容性问题 | 中 | 低 | 锁定版本，CI 测试多 Rust 版本 |

---

## 资源需求

### 人力资源
- Rust 开发工程师: 2-3 人
- C/C++ 工程师 (支持): 1 人
- DevOps 工程师 (兼职): 1 人

### 硬件资源
- 开发服务器: 16 核 32GB RAM
- 测试集群: 3 节点 Kubernetes
- CI/CD: GitHub Actions/GitLab CI

### 软件工具
- Rust: 1.75+
- CMake: 3.20+
- Docker: 24+
- Kubernetes: 1.28+

---

**最后更新**: 2025-01-XX  
**版本**: v1.0  
**审批**: Qwen3-TTS Tech Lead
