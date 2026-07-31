# Qwen3-TTS Rust 服务端文档索引

欢迎查阅 Qwen3-TTS Rust 服务端开发文档。本文档集涵盖了项目架构、开发指南和路线图。

## 📚 文档列表

### 1. [README.md](./README.md) - 项目总览与任务清单
**适合读者**: 所有项目参与者

**主要内容**:
- 项目概述与架构目标
- ✅ 已完成任务清单
- 📋 待完成任务 (分 4 个阶段)
- 技术栈说明
- 构建与运行指南
- 性能目标

**快速链接**:
- [已完成任务](./README.md#已完成任务)
- [待完成任务](./README.md#待完成任务)
- [构建命令](./README.md#构建与运行)

---

### 2. [FFI_GUIDE.md](./FFI_GUIDE.md) - FFI 绑定实现指南
**适合读者**: Rust 开发工程师、C/C++ 工程师

**主要内容**:
- FFI 基础概念与最佳实践
- C/Rust 类型映射表
- 内存安全实践 (RAII、生命周期)
- 错误处理模式
- 完整示例代码
- 调试技巧与性能优化

**快速链接**:
- [类型映射表](./FFI_GUIDE.md#类型映射)
- [RAII 封装示例](./FFI_GUIDE.md#raii 封装)
- [错误处理模式](./FFI_GUIDE.md#错误处理模式)
- [流式响应示例](./FFI_GUIDE.md#流式响应实现)

---

### 3. [ROADMAP.md](./ROADMAP.md) - 开发路线图
**适合读者**: 项目经理、技术负责人、开发工程师

**主要内容**:
- 4 个开发阶段详细规划
- Sprint 任务分解与验收标准
- 里程碑检查点 (M1-M4)
- 风险评估与缓解措施
- 资源需求估算

**快速链接**:
- [Phase 1: FFI 完善](./ROADMAP.md#phase-1-完善 ffi 绑定高优先级)
- [Phase 2: 功能增强](./ROADMAP.md#phase-2-服务功能增强中优先级)
- [里程碑检查点](./ROADMAP.md#里程碑检查点)

---

## 🗺️ 阅读路径建议

### 新加入项目的开发者
```
README.md → FFI_GUIDE.md (第 1-3 章) → ROADMAP.md (Phase 1 任务)
```

### C 语言工程师 (参与 FFI 对接)
```
README.md (架构目标) → FFI_GUIDE.md (全文) → 开始实现任务
```

### 项目经理/技术负责人
```
README.md (总览) → ROADMAP.md (全文) → 定期review任务进度
```

---

## 📊 当前项目状态

| 维度 | 状态 | 说明 |
|------|------|------|
| 项目阶段 | Phase 1 初期 | FFI 基础框架已搭建 |
| 完成度 | ~15% | 基础架构完成，核心 FFI 待实现 |
| 下一里程碑 | M1: FFI 完成 | 预计 4 周后 |
| 风险等级 | 🟢 低 | 按计划推进 |

---

## 🔧 快速开始

### 1. 环境准备
```bash
# 安装 Rust (>= 1.75)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 验证安装
rustc --version
cargo --version
```

### 2. 克隆项目
```bash
git clone <repository-url>
cd qwen-tts-rust
```

### 3. 编译项目
```bash
# 调试版本
cargo build

# 发布版本 (推荐生产环境)
cargo build --release
```

### 4. 运行测试
```bash
# 运行单元测试
cargo test

# 运行带日志的测试
RUST_LOG=debug cargo test
```

### 5. 启动服务
```bash
# 设置环境变量
export QWEN_TTS_MODEL_PATH="/path/to/model"
export RUST_LOG=info

# 运行服务
cargo run --release
```

---

## 📝 贡献指南

### 提交代码前检查清单
- [ ] 代码通过 `cargo clippy` 检查
- [ ] 代码通过 `cargo fmt` 格式化
- [ ] 新增代码有单元测试
- [ ] 更新相关文档
- [ ] 通过 CI/CD 流水线

### 文档更新流程
1. 在对应 `.md` 文件中修改内容
2. 更新文档底部的"最后更新"日期
3. 如有必要，更新其他文档中的交叉引用
4. 提交时包含 `[docs]` 标签

---

## 🤝 获取帮助

### 常见问题
- 查看 [FFI_GUIDE.md](./FFI_GUIDE.md#常见问题) 的 FAQ 章节
- 查看 GitHub Issues 中的已解决问题

### 联系方式
- 项目讨论：GitHub Discussions
- Bug 报告：GitHub Issues
- 紧急问题：联系项目维护者

---

## 📅 文档更新记录

| 日期 | 文档 | 更新内容 | 作者 |
|------|------|----------|------|
| 2025-01-XX | README.md | 初始版本，任务清单 | Qwen3-TTS Team |
| 2025-01-XX | FFI_GUIDE.md | 初始版本，FFI 最佳实践 | Qwen3-TTS Team |
| 2025-01-XX | ROADMAP.md | 初始版本，开发路线图 | Qwen3-TTS Team |
| 2025-01-XX | INDEX.md | 初始版本，文档索引 | Qwen3-TTS Team |

---

## 📌 关键决策记录 (ADR)

重要的架构决策将记录在 [`docs/adr/`](./adr/) 目录中 (待创建)。

### 已确定的决策
1. **使用 Axum 作为 HTTP 框架**
   - 理由：Tokio 生态原生支持，性能优秀，API 设计现代
   - 日期：2025-01-XX

2. **C 库作为静态库链接**
   - 理由：简化部署，避免动态库版本问题
   - 日期：2025-01-XX

3. **异步优先架构**
   - 理由：充分利用 Rust async/await，提高并发性能
   - 日期：2025-01-XX

---

**维护者**: Qwen3-TTS Team  
**许可证**: Apache 2.0 / MIT  
**最后更新**: 2025-01-XX
