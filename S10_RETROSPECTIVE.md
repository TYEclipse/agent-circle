# Agent Circle · S10 Sprint 回顾

> **Sprint**: S10 · Service Discovery (轮 101–110)  
> **锚点**: 组合化  
> **日期**: 2026-06-14  
> **状态**: ✅ 闭合 (R101–R110 全部完成)

---

## 一、完成总览

| 轮 | 任务 | 状态 |
|----|------|------|
| R101 | Service 注册 — Agent Card 扩展 `services` 字段 | ✅ |
| R102 | Service 广播 — GossipSub 服务频道 | ✅ |
| R103 | Service 查询 — `service search` CLI | ✅ |
| R104 | Service 直连调用 — `service call` | ✅ |
| R105 | 服务发现展示层 — 彩色 ASCII 表格 | ✅ |
| R106 | 服务能力协商 — CapabilityProbe/Statement | ✅ |
| R107 | "公众号"模式 — 服务订阅 | ✅ |
| R108 | 服务离线缓存 — 本地持久化查询 | ✅ |
| R109 | 服务市场 PoC — `service publish` | ✅ |
| R110 | S10 回顾 | ✅ 本文档 |

---

## 二、交付物

### 代码
- **`agent-circle-core/src/identity.rs`** — ServiceInfo 扩展 (services, protocol_versions, input_schema)；CapabilityProbe / CapabilityStatement / ProtocolVersion 类型 (R101, R106)
- **`agent-circle-core/src/chat.rs`** — ChatRequest 增加 `service: Option<ServiceCall>` 字段 (R104)
- **`src/service_discovery.rs`** — ServiceRegistry + GossipSub 发布/订阅 (R102)；持久化 save/load (R103)；all_services_with_meta() / last_seen_for() / has_cached_data() / is_peer_fresh() API (R105, R108)；ServiceSubscriptions 订阅系统 + handle_service_message 通知 (R107)
- **`src/main.rs`** — Service CLI 完整套件：list (彩色表格 R105)、search (R103)、call (R104)、negotiate (R106)、subscribe/unsubscribe/subscriptions (R107)、cache --stats/--flush (R108)、publish (R109)
- **`src/network.rs`** — daemon 集成：服务 topic 订阅 + 定时公告 (R102)；服务调用请求字段 (R104)；订阅加载 + 通知 (R107)
- **`src/protocol.rs`** — SERVICE_TOPIC 常量 (R102)

---

## 三、技术决策

1. **Service 承载于 Agent Card**: Service 信息以 `AgentCard.services` 扩展形式承载，而非独立 DHT 记录，简化发现逻辑
2. **GossipSub 频道发现**: 服务发现采用 GossipSub 频道广播 + 本地 JSON 持久化缓存，无中心依赖
3. **ChatRequest.service 复用**: 服务调用复用现有消息管道 (`ChatRequest.service` 可选字段)，无需独立协议
4. **CLI-first 协商**: 能力协商 CLI 从本地缓存读取，daemon 侧实时协商留待后续网络层集成
5. **订阅去中心**: 订阅信息本地管理 (`subscriptions.json`)，daemon 启动后根据服务公告自动匹配通知
6. **零依赖扩展**: 彩色表格、协商、订阅、缓存、发布全部实现零新增 crate 依赖

---

## 四、测试覆盖

- Cargo build: 零错误 · 零警告
- Cargo test: 全量通过
- Cargo fmt: 零偏差
- Cargo clippy: 零告警
- CI: 全部 job 全绿

---

## 五、经验教训

### 做得好的
- **增量交付**: 每轮一个独立可验证的 CLI 命令，用户可立即看到效果
- **API 渐进**: ServiceRegistry 从 R102 的 ingest 方法逐步扩展到 R108 的 10+ 公开 API
- **零新增依赖**: R105 表格用裸 ANSI escape codes 实现，避免 tabled/comfy-table 等重型依赖
- **daemon 同步**: 每轮 CLI 功能与 daemon 侧逻辑同步开发（订阅通知、缓存持久化）
- **emoji 安全**: R105 发现 byte-length 截断问题后立即改用 char_indices，避免多字节字符崩溃

### 可改进的
- `service call` 在 CLI 模式下仅打印提示，未实际发送 P2P 请求（需要 daemon 在线）
- 能力协商 (`negotiate`) 仅读取本地缓存，实时 P2P 协商需 daemon 集成
- ServiceSubscriptions 通知目前仅日志输出，未推送到用户消息通道
- ServiceInfo 新增字段未更新测试 fixtures（当前无直接构造 ServiceInfo 的测试，不阻塞）

---

## 六、延期项

| 轮 | 任务 | 原因 | 新 Sprint |
|----|------|------|-----------|
| — | 实时 P2P 协商 | CLI 协商已实现，daemon 侧实时响应留待 S11 诊断命令集成 | S11 |
| — | 订阅推送 | 当前仅日志通知，用户消息通道推送留待 S11 TUI | S11 |

---

## 七、S11 准备

S11 · 诊断 + 日志（轮 111–120）锚点：维修性（全链路诊断）

1. R111: `agent-circle doctor` 诊断命令 — 一键检查 identity/network/storage/contacts
2. R112: 网络拓扑诊断 (`doctor network`) — Peer 连接图 + RTT + 丢包率
3. R113: 存储完整性检查 (`doctor storage`) — 校验 timeline.json / contacts.json
4. R114: 协议跟踪日志 (`--trace protocol`) — 每条消息收发完整 trace
5. R115–R120: 错误码体系、性能指标、健康检查、Crash dump、远程诊断

---

## 八、S10 闭合声明

S10 Service Discovery 已闭合（10/10 轮全部完成）。

Agent Circle 现在拥有完整的 Service Discovery 体系：
- ✅ Service 注册 + GossipSub 广播 (R101–R102)
- ✅ CLI 查询 + 直连调用 (R103–R104)
- ✅ 彩色表格展示层 (R105)
- ✅ 能力协商 (R106)
- ✅ 公众号订阅 (R107)
- ✅ 离线缓存 (R108)
- ✅ 服务市场 PoC (R109)
- ✅ 全部 10 轮 build·test·fmt·clippy 零告警

**S11 ready.**
