# S11 回顾 · 诊断 + 日志

**日期**：2026-06-14  
**轮次**：111–120  
**锚点**：维修性（全链路诊断）

## 完成情况

| 轮 | 任务 | 验收 | 状态 |
|---|---|---|---|
| R111 | `agent-circle doctor` 诊断命令 | 一键检查 identity/network/storage/contacts | ✅ |
| R112 | 网络拓扑诊断（`doctor network`） | Peer 列表 + 🟢🟡🔴 新鲜度标注 | ✅ |
| R113 | 存储完整性检查（`doctor storage`） | 校验 card/contacts/timeline/services.json | ✅ |
| R114 | 协议跟踪日志 | 每条消息收发完整 trace | ⏳ 延后（需 daemon 侧改动） |
| R115 | 错误码体系 | 5 个错误码 E0001–E0005 + 查找 API | ✅ |
| R116 | 性能指标暴露（Prometheus） | `agent-circle metrics` OpenMetrics 输出 | ✅ |
| R117 | 健康检查 HTTP 端点 | `curl :9099/health` + `/metrics` | ✅ |
| R118 | Crash dump 系统 | `~/.agent-circle/crash/<iso>.dump` JSON | ✅ |
| R119 | 远程诊断模式 | `doctor --peer <ID>` 远程自检 | ✅ |
| R120 | 回顾 | 本文档 | ✅ |

**闭合**：9/10 完成，R114 协议跟踪日志合理延后 —— 需要 daemon 侧变更，与 S12 保障性工作有重叠。

## 关键成果

### 维修性矩阵已建立

```
           ┌─ 本地自查  → doctor (R111)
           ├─ 存储校验  → doctor storage (R113)
           ├─ 网络诊断  → doctor network (R112)
维修性 ───┼─ 错误编码  → E0001–E0005 (R115)
           ├─ 指标暴露  → /metrics (R116) + :9099 (R117)
           ├─ 崩溃取证  → crash dump (R118)
           └─ 远程诊断  → doctor --peer (R119)
```

### 新增模块一览

| 模块 | 轮 | 行数 | 功能 |
|------|------|------|------|
| `src/metrics.rs` | R116 | 220 | OpenMetrics 输出 + collect_for_dir |
| `src/health.rs` | R117 | 140 | tokio HTTP 服务器 (零新依赖) |
| `src/crash.rs` | R118 | 195 | panic hook → JSON dump |
| `src/network.rs` (增量) | R119 | +130 | DoctorBehaviour + run_doctor_checks |
| `agent-circle-core/src/chat.rs` (增量) | R119 | +35 | DoctorRequest / DoctorResponse |

**净新增代码**：~550 行 Rust，4 个质量关卡全绿

## 关键决策

- **健康检查 HTTP 服务器**使用 `tokio::net::TcpListener` + 原生 HTTP/1.1 解析，引入零新依赖 —— 避免引入 hyper/axum 带来的传递依赖膨胀
- **控制端口检测**从 `control.sock` 改为 `control.port`（三处统一修正：metrics / health / doctor）—— 与实际控制服务器写入的文件名一致
- **R114 协议跟踪日志**延后而非强行交付 —— 需要 daemon 侧的 `--trace` 标志和每个 SwarmEvent 的 tracing span，与 S12 保障性工作重叠区域大，在 S12 交付更合理
- **远程诊断**复用本地 `cmd_doctor` 的检查逻辑，但以简化版实现（快速文件存在性检查 + 计数）避免 JSON 序列化完整 doctor 输出的复杂性
- **Crash dump** 使用 `std::backtrace::Backtrace::force_capture()` 而非 tokio 的 task-local backtrace —— 因为 panic hook 运行在 unwind 上下文中，tokio 的 task 概念此时可能已失效
- **DoctorBehaviour** 复用 libp2p `request_response::json::Behaviour`，与 `ChatBehaviour` 模式完全一致，没有引入新的网络抽象

## 学到的事

1. **libp2p request-response 模式可复用**：ChatBehaviour 的模式（`json::Behaviour<T, U>`）可以直接套用到任何新的请求-响应协议。新协议只需：(1) 定义 serde 类型，(2) 注册 StreamProtocol，(3) 添加 NetworkBehaviour 字段
2. **`control.port` vs `control.sock` 是一类隐蔽 bug**：三处代码（两处在 R116/R117 新建，一处在 R112 遗产）都写了 `control.sock`，与实际 `control.port` 文件名不一致。在 daemon 实际运行前无法发现 —— 说明这类路径常量应该集中定义
3. **HTTP 服务器不用框架也够用**：15 个 gauge 指标的 /health + /metrics，用原生 TCP 解析约 140 行实现，零新依赖。对于 agent 内嵌的简单 HTTP 服务，引入 hyper 的代价不值得
4. **Crash dump 的价值在于事后分析**：JSON 格式 + agent 状态快照 + backtrace 的组合，让非本地复现的 panic 也能追溯上下文。但当前只在 CLI/daemon 崩溃时触发，Tokio task panic 仍需 `AssertUnwindSafe` 包裹
5. **远程诊断是维修性能力的乘法器**：本地 doctor 只在单节点有用；远程 doctor 让运维者可以拓扑范围内批量自检，与 /metrics Prometheus 刮取形成互补（pull 指标 vs push 诊断）
6. **S11 是首个"跨轮依赖"明显的 Sprint**：R116 metrics 被 R117 HTTP 端点直接复用（`collect_for_dir`），R119 远程诊断复用了 R111 的检查逻辑。这说明模块化设计开始产生回报

## S12 展望

S11 建立了完整的诊断体系（自查 + 远程 + 指标 + 崩溃取证 + 健康检查）。S12 的锚点是**保障性**（一键部署 + CI/CD），当前任务包括：

- R121–R124: 用户手册、API 文档、协议规范、贡献指南
- R125–R129: crates.io 发布、.deb/.rpm 打包、Homebrew、Docker
- R130: S12 回顾

**累计进度**：119/200 (59.5%)，S11 闭合
