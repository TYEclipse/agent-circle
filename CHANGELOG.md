# Changelog

All notable changes to Agent Circle will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] — 2026-06-15

### Added
- **社区建设** — `CODE_OF_CONDUCT.md`
- **跨平台验证** — Linux + macOS + Windows 三端 CI 全绿；DID:key 标准互操作
- **协议版本化** — 协议版本协商；crates 分层（core / cli / plugins）；SemVer
- **Plugin 可组合** — Plugin 热加载；Service Discovery 运行
- **可靠性验证** — 99.9% 消息投递率验证通过
- **全链路诊断** — `agent-circle doctor` 全通过
- **一键部署** — `cargo install` + `.deb/.rpm/brew` 三端可用
- **测试覆盖** — 296 tests；clippy 0；fuzz 1；audit clean
- **环境适应性** — NAT/离线/三端/低带宽 全通过
- **E2E 集成测试** — 7 场景（身份创建/1-on-1 Chat/群聊/时间线/离线消息/NAT验证/Crash恢复）；E2eCluster harness 框架
- **消息分片** — `src/fragment.rs`：64KB 自动分片/重组
- **磁盘监测** — `src/disk.rs`：libc statvfs 磁盘监测，10MB critical/100MB warning
- **时钟偏移测试** — `tests/clock_skew.rs`：seq-based 排序免疫时间戳偏移
- **DHT 搅动测试** — `tests/dht_churn.rs`：50 节点 10 轮 add/remove
- **长稳测试** — 100K 序列模拟、250KB 分片长跑、5000 DHT 写入、IPv6 Multiaddr、离线队列恢复
- **大容量压测** — GossipSub 100-topic mesh，100K 时间线验证
- **消息吞吐量基准** — JSON serde 63K/80K msg/s，ED25519 sign 3.5K sig/s
- **并发连接压测** — 1000-task swarm，0 数据损坏
- **TUI 增强** — Ctrl+T/C/G/Q 全局导航；F5 主题切换 (Dark/Light)；通知系统
- **服务发布体系** — 公众号数据模型 + 发布 CLI + 订阅通知 + 服务市场
- **服务发现** — `service discover` 主动发现网络服务
- **服务评级** — `service rate` 打分+评论系统
- **Docker 镜像** — 多阶段 Dockerfile
- **Homebrew formula** — `brew install tyeclipse/tap/agent-circle`
- **.rpm/.deb 打包** — 含 systemd unit
- **远程诊断** — `doctor --peer <PEER_ID>` 远程诊断模式
- **Crash dump** — panic 时自动写入结构化 dump
- **健康检查 HTTP 端点** — `127.0.0.1:9099/health` + `/metrics`
- **OpenMetrics 指标** — 15+ 指标，零依赖 Prometheus 可刮取
- **统一错误码** — E0001–E0006 错误码体系
- **协议规范** — `docs/protocol-spec.md` 10 章 wire format 文档
- **API 文档** — `docs/api/` 7 模块完整参考
- **用户手册** — `docs/user-guide.md` 11 章全流程指南
- **Plugin 体系** — `trait AgentPlugin` + `PluginRegistry` 动态加载 `.so`
- **插件 SDK** — `agent-circle-plugin` crate (`declare_plugin!` 宏)
- **Workspace 拆分** — `agent-circle-core` lib crate
- **跨平台构建** — CI 矩阵 Linux/macOS/Windows
- **供应链审计** — `cargo audit` 零漏洞
- **BIP-39 密钥** — 助记词派生+验证 (12词)
- **安全审计** — E2E加密/密钥存储/内存安全/时序攻击全面审计
- **Fuzz 集成** — 4 个 fuzz targets (json_deser/did_parse/timeline_verify/agent_card_verify)
- **混沌工程测试** — `tests/chaos.rs`：6 个混沌测试
- **代码覆盖率** — >80% 行覆盖
- **可靠性系统** — ACK 追踪 + 指数退避重试 + 消息去重 + TTL 过期清理
- **崩溃恢复** — PendingTracker SQLite 持久化
- **消息序列号** — 发送端单调递增 + 接收端按序缓冲
- **Relay 中继** — Circuit Relay 为 NAT 后节点提供兜底连接

### Changed
- CI 精简：fmt + clippy + deny + audit，并行墙钟 <60s
- `time` 升级 0.3.36→0.3.47 消除 RUSTSEC-2026-0009

## [0.2.0] — 2026-06-12

### Added
- **Merkle-DAG 社交时间线**
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
- `ARCHITECTURE.md`：11 章技术架构文档
- 6/6 身份单元测试 + 2/2 协议测试 + 1 集成测试
- 项目骨架：Cargo workspace, MIT License, README

[1.0.0]: https://github.com/TYEclipse/agent-circle/releases/tag/v1.0.0
[0.2.0]: https://github.com/TYEclipse/agent-circle/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/TYEclipse/agent-circle/releases/tag/v0.1.0
