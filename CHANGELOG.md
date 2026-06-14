# Changelog

All notable changes to Agent Circle will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **S10R105: Service 彩色表格展示层** — `agent-circle service list` 输出 ANSI 彩色 ASCII 表格（Peer/Service/Name/Endpoint/Tags 列）；`--verbose` 模式显示 Description + 最后在线时间；`ServiceRegistry.all_services_with_meta()` API 扩展（含 `last_seen` 时间戳）；字符边界安全的截断支持 emoji
- **S09R91–R99: Plugin 体系 ✅** — 插件接口 `trait AgentPlugin` (5 生命周期钩子 + 消息处理 + CLI)，`PluginRegistry` 动态加载 `.so`，`plugin list` CLI；**插件 SDK** `agent-circle-plugin` crate (`declare_plugin!` 宏 + re-export)；**内置 hello-world 插件** (cdylib + workspace 成员，匹配 hello/你好)；**Plugin 安全模型文档** (`docs/plugin-security.md`：威胁模型、能力声明、用户授权流程、WASM 沙箱方案)
- **S08R81–R86: 协议版本化 + workspace 拆分** — `src/protocol.rs` 集中版本常量 (VERSION, chat/relay/group protocols)；`SUPPORTED_CHAT_PROTOCOLS` 多版本兼容规划；`docs/protocol-versioning.md` SemVer 策略文档；workspace Cargo.toml + `agent-circle-core` lib crate (shared types: chat, identity, errors, keys, protocol)；main crate re-exports from core via stub modules
- **S07R71–R80: 跨平台构建 ✅** — CI 矩阵扩展到 Linux/macOS/Windows (`build-and-test` job)；跨平台路径 (`AGENT_CIRCLE_HOME` env var)；control socket 替代 SIGUSR1 (`daemon log-level <LEVEL>` 全平台通用)；`daemon install`/`uninstall` 一键生成 systemd/launchd/WinSW 服务配置（自动检测平台）；WinSW XML 模板 + 服务文档 (`services/README.md`)
- **S06R61–R66: 供应链审计 + BIP-39 密钥** — `SUPPLY_CHAIN.md` 依赖审计（807 transitive，3 vuln豁免）；`src/keys.rs` BIP-39 助记词派生+验证（12词，PBKDF2→Ed25519），`identity mnemonic` / `identity restore` CLI；5个keys测试（含已知测试向量）
- **S05R51–R59: 安全审计** — `SECURITY_AUDIT.md`：E2E加密/密钥存储/内存安全/时序攻击/消息签名/重放防护全面审计；综合评分 87.5%；R57 连接限制已实现 (`memory-connection-limits`，流入50/流出50/待定10)
- **S04R50: S04 回顾文档** — `S04_RETROSPECTIVE.md`：fuzz+混沌总览、测试数据、经验教训、S05 准备
- **S04R47–R49: 混沌工程测试** — `tests/chaos.rs`：6 个混沌测试 (崩溃恢复持久化 + 部分交付恢复 + 网络分区离线队列 + 消息洪峰 200 条 + 洪峰半排半留)；验证 Queue SQLite 持久化在 crash/partition/flood 下不丢消息
- **S04R41–R46: cargo fuzz 集成 + 4 个 fuzz targets** — `fuzz/`：`cargo +nightly fuzz` 脚手架，4 个 fuzz target：json_deser (ChatRequest/ChatResponse/AgentCard/TimelineNode/Timeline 反序列化)、did_parse (decode_did_key)、timeline_verify (Timeline::verify)、agent_card_verify (AgentCard::verify)；所有 target 保证任意输入不 panic
- **S03R40: S03 回顾文档** — `S03_RETROSPECTIVE.md`：覆盖率总览、经验教训、风险与延期、S04 准备清单
- **S03R39: 测试数据工厂 (TestFixtures)** — `tests/common/fixtures.rs`：random_identity、seeded_identity、valid_chat_request、chat_request_seq、empty/zeroed_chat_request、random_agent_card、agent_card_for、genesis_node、empty/multi_node_timeline、invalid_did、malformed_signature；集成测试已重构使用 fixtures
- **S03R36: timeline.rs 测试覆盖 → 100%** — 补充 15 个测试：空/默认 timeline、空验证、空追加、确定性 hash、不同内容不同 hash、id/parent 篡改检测、serde 往返、内部 hash_node/signing_payload 函数、len 计数、跨身份签名伪造检测
- **S03R35: chat.rs 测试覆盖 → 100%** — 补充 9 个测试：new_msg_id 非零+唯一性(100个无碰撞)、default_ttl 未来时间戳、ChatRequest/ChatResponse serde 往返、Debug/Clone/可选字段零值/额外字段兼容
- **S03R37/R38: Mock swarm 框架 + 集成测试** — `tests/common/` MockNode 进程内 P2P 节点，自动 ACK chat 消息；2 个集成测试 (单消息投递 + 多消息全部确认) 验证 end-to-end 消息流，无需真实 daemon
- **S03R32: identity.rs 测试覆盖 → 100%** — 补充 9 个测试：decode_did_key 错误路径 (bad prefix/base58/multicodec/wrong length)、agent_card.verify 错误路径 (invalid proof encoding/length)、from_seed 确定性、to_seed_bytes 长度、verifying_key
- **S03R33: storage.rs 测试覆盖 → 100%** — 补充 11 个测试：resolve_data_dir (default/override)、identity save/load (roundtrip/missing/wrong size)、card save/load、contacts add/list/duplicate、timeline save/load (roundtrip/missing)
- **S02R24: 投递状态回调** — `chat send --track` 发送后等待 ACK/Failure 实时打印投递状态（✅ Delivered / ❌ Failed / ⏰ Pending）；`--timeout` 自定义超时（默认 30s）；`send_chat()` 现在返回 `OutboundRequestId`
- **S02R22: 消息序列号 + 顺序保证** — `ChatRequest.seq` 发送端单调递增，`SequenceTracker` 接收端按序缓冲乱序消息，gap 填满后自动冲刷投递；断线重连时重置 per-peer 状态；新增 6 个单元测试
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
