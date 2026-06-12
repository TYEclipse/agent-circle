# S00R9 · 代码基线报告

**日期**：2026-06-13  
**分支**：master  
**版本**：v0.1.0 (S00 工程基座)

## 代码量

| 指标 | 值 |
|---|---|
| Rust 源代码文件 | 10 |
| Rust 代码行 | 1,625 |
| 测试覆盖 | 22 测试 (11 lib + 11 main) |
| 集成测试 | 1 (#[ignore]) |
| Markdown 文档 | 4 文件 (README / ARCHITECTURE / ROADMAP / CHANGELOG) |

## 文件分布

| 文件 | 行数 | 职责 |
|---|---|---|
| `main.rs` | ~740 | CLI 入口 + 命令分发 + subsciber 初始化 |
| `network.rs` | ~305 | libp2p swarm + 事件循环 + chat + GossipSub |
| `identity.rs` | ~509 | DID:key + Agent Card + Ed25519 签名 |
| `timeline.rs` | ~180 | Merkle-DAG 朋友圈 |
| `storage.rs` | ~90 | 文件持久化 (identity/contacts/timeline) |
| `chat.rs` | ~40 | 聊天协议消息体 |
| `protocol.rs` | ~40 | 握手协议消息体 |
| `errors.rs` | ~30 | 错误类型定义 |
| `lib.rs` | ~5 | 库入口 |
| `tests/gossipsub_integration.rs` | ~90 | 集成测试 |

## 质量门禁

| 门禁 | 状态 |
|---|---|
| `cargo fmt` | ✅ |
| `cargo clippy` (CI strict) | ✅ 0 warnings |
| `cargo test` | ✅ 22/22 |
| `cargo deny` | ✅ licenses + bans + sources |
| `cargo audit` | ⚠️ 3 已知漏洞 (传递依赖 → S05-S06) |
| CI pipeline | ✅ 6 jobs (fmt/clippy/test/build-release/deny/audit) |

## 工程工具

| 工具 | 状态 |
|---|---|
| `just` task runner | ✅ 24 recipes |
| `scripts/release.sh` | ✅ |
| `CHANGELOG.md` | ✅ Keep a Changelog |
| `ROADMAP.md` | ✅ 20 Sprint × 200轮 |
| `deny.toml` | ✅ |
| `.github/workflows/ci.yml` | ✅ |
