# Changelog

All notable changes to Agent Circle will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **S02R19: 崩溃恢复 — PendingTracker SQLite 持久化** — `pending` 表记录所有飞行中消息，daemon 被 kill -9 后重启自动恢复未 ACK 消息并重新发送；Queue 统一打开一次复用；expire_pending() 定期清理过期 pending 条目；新增 6 个单元测试
- **S02R18: 消息 TTL 与过期清理** — `ChatRequest.ttl`（默认 7 天），离线队列 `expire_before()` 自动清理过期消息；daemon 每 5 分钟自动 `prune_delivered()` + `expire_before()`；`agent-circle diag clean` CLI 手动清理
- **S02R17: 全链路诊断** — `DiagCounters` 原子计数器追踪每条消息生命周期（发送/ACK/重试/失败/入队/重复），daemon 每 30s 自动输出送达率统计；`agent-circle diag queue` CLI 查看离线队列
- **S02R16: 消息去重** — `DedupFilter` 按 `msg_id` 追踪已收消息，重传自动去重只发 ACK 不重复处理，结合 ACK+重试实现 effectively-once 语义
- **S02R15: 消息可靠性 — ACK 追踪 + 指数退避重试** — `PendingTracker` 按 `OutboundRequestId` 追踪飞行中消息，ACK 到达确认送达，传输失败自动重试（最多 3 次），重试耗尽降级离线队列
- **S01R13: Relay 发现协议** — relay 节点通过 DHT 广播地址（`/agent-circle/relays/0.1.0`），新节点启动后自动查询 DHT 发现并拨号 relay

### Changed
- **CI 精简** — 本地跑全量 CI，线上只留 fmt(~10s) + clippy(~45s) + deny(~10s) + audit(~15s)，并行墙钟 <60s
- **Security** — `time` 升级 0.3.36→0.3.47 消除 RUSTSEC-2026-0009；hickory-proto/paste/lru 传递依赖漏洞纳入 audit.toml 豁免（libp2p 暂不可升）
- **S01R12: Relay 中继节点实现** — `relay::Behaviour` 集成，节点可作为 Circuit Relay 为 NAT 后节点提供兜底连接
- `daemon start --relay` CLI flag 启用以太坊中继模式
- CI/CD pipeline (GitHub Actions)：fmt → clippy → test → build-release → deny → audit
- Structured logging：`tracing` + JSON 输出 (daemon 模式)
- Dynamic log level switching：SIGUSR1 热切换 (error↔warn↔info↔debug↔trace)
- License audit：`cargo-deny` (bans + licenses + sources)
- Security audit：`cargo-audit` (advisory mode)
- `cargo fmt` 全量统一 + CI 门禁
- `deny.toml` 许可证白名单 (MIT / Apache-2.0 / ISC / BSD / MPL-2.0)

### Changed
- 工程目标定稿：200 轮敏捷迭代 → 三化六性 P2P Agent 社交基础设施

## [0.2.0] — 2026-06-12

### Added
- **Merkle-DAG 社交时间线（朋友圈）**
  - Genesis 创始帖创建
  - 追加新帖（哈希链链接）
  - 完整链验证（Hash + Ed25519 签名）
  - 篡改检测（Content / Signature / Hash-chain 三种攻击）
  - CBOR 序列化
  - CLI：`timeline genesis | post | show | verify`
- 3/3 时间线单元测试

## [0.1.0] — 2026-06-12

### Added
- **身份系统**：DID:key (Ed25519) + 自签名 Agent Card + 短码
- **联系人**：本地加密存储 (contacts.json)
- **1 对 1 P2P 聊天**：libp2p QUIC + Noise 加密 + request_response ACK
- **GossipSub 群聊**：mesh 形成 + 话题订阅 + 消息广播
- **P2P 网络**：libp2p Swarm (QUIC + TCP, mDNS, Kademlia DHT, DCUtR, Identify)
- **握手协议**：Hello / Challenge / Accept / Ack (CBOR)
- CLI (clap derive)：`identity | daemon | contact | chat | group`
- 目标一句话定稿："AI 智能体的微信——..."
- `ARCHITECTURE.md`：11 章技术架构文档
- 6/6 身份单元测试 + 2/2 协议测试 + 1 集成测试 (#[ignore])
- 项目骨架：Cargo workspace, MIT License, README

[Unreleased]: https://github.com/TYEclipse/agent-circle/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/TYEclipse/agent-circle/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/TYEclipse/agent-circle/releases/tag/v0.1.0
