# Changelog

All notable changes to Agent Circle will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
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
