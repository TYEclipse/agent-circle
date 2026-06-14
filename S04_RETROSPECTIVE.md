# S04 Sprint 回顾 — Fuzz + 混沌工程

**日期**：2026-06-14  
**轮次**：R41–R50  
**Sprint 目标**：测试性锚点 → fuzz 集成 + 混沌测试

---

## 完成情况

| 轮 | 任务 | 状态 | 交付 |
|----|------|------|------|
| R41 | `cargo fuzz` 集成 | ✅ | fuzz/ 脚手架 |
| R42 | CBOR 消息反序列化 fuzz | ✅ | json_deser (JSON 替代 CBOR) |
| R43 | DID 解析 fuzz | ✅ | did_parse (decode_did_key) |
| R44 | 握手协议 fuzz | ⏭️ | protocol.rs 未编译，随 S09 补 |
| R45 | timeline verify fuzz | ✅ | timeline_verify |
| R46 | agent card verify fuzz | ✅ | agent_card_verify |
| R47 | 崩溃恢复混沌 | ✅ | 3 个 crash recovery tests |
| R48 | 网络分区混沌 | ✅ | 离线队列测试 |
| R49 | 消息洪峰混沌 | ✅ | 200-msg flood + drain |
| R50 | S04 回顾 | ✅ | 本文档 |

**交付**：84 测试（76 ut + 6 chaos + 2 it），4 fuzz targets，零 clippy。

---

## Fuzz 概况

| Target | 输入 | 断言 |
|--------|------|------|
| json_deser | 任意 bytes | ChatRequest/ChatResponse/AgentCard/TimelineNode/Timeline 反序列化不 panic |
| did_parse | 任意 bytes | decode_did_key 不 panic |
| timeline_verify | 任意 JSON Timeline | verify() 不 panic |
| agent_card_verify | 任意 JSON AgentCard | verify() 不 panic |

运行方式：`cargo +nightly fuzz run <target>`

---

## 混沌测试概况

6 个混沌测试覆盖 Queue 层的极端条件：

| 测试 | 场景 | 验证 |
|------|------|------|
| crash recovery 持久化 | 推入 10 条 → drop Queue → 重开 | 10 条全部存活 |
| 空队列恢复 | 首次打开空队列 | pending 为空，不报错 |
| 部分交付崩溃 | 10 条推入 → 前 3 标记已交付 → 崩溃 → 重开 | 7 条未交付 |
| 离线队列 | 推入 5 条 | 接收方上线后全部可见 |
| 洪峰 200 条 | 紧密循环写入 200 条 | 全部入队，顺序正确 |
| 洪峰半排 | 50 条推入 → 前 25 已交付 | 后 25 仍 pending |

---

## 经验教训

### 做得好的
1. **API 驱动测试**：chaos tests 直接使用 Queue API，不依赖 daemon，快速、可重复。
2. **fuzz targets 设计**：所有 target 遵循"任意输入 → 不 panic"原则，覆盖了序列化边界。
3. **崩溃恢复测试分层**：从纯持久化到部分交付逐步加深，验证 SQLite 的 ACID 保证。

### 待改进
1. **R44 握手协议 fuzz 跳过了**：protocol.rs 引用 `ciborium`（未在 deps 中）且不在 lib.rs，无法编译。需在 S09 插件体系阶段处理。
2. **R42 CBOR → JSON**：项目文档记的是 CBOR，但实际代码用 serde_json。CBOR 依赖 `ciborium` 未引入。ROADMAP 应同步。
3. **fuzz build 慢**：`cargo +nightly fuzz build` 在 WSL 上需要 >5min（ASAN 插桩），CI 不适合。建议本地单独执行。
4. **fuzz corpus 空白**：未采集种子输入以加速覆盖。随 fuzz 运行会自动积累。

---

## 风险与延期

| 风险 | 等级 | 处置 |
|------|------|------|
| protocol.rs/ciborium 不存在 | 🟡 低 | 跳 R44，随 S09 一起处理和 ROADMAP 更新 |
| fuzz build 超 5min | 🟡 低 | 本地执行，不入 CI |
| GitHub 漏洞告警 12 条 | 🟠 中 | 均为传递依赖，延续 audit.toml 豁免策略 |

---

## 测试总览（截至 S04）

```
84 tests: 76 ut + 6 chaos + 2 it (1 ignored)
4 fuzz targets, 14 fixtures
CI: fmt(10s) + clippy(45s) + deny(10s) + audit(15s) < 60s ✅
```

---

## 进入 S05 的准备

- [x] 交付 push
- [x] CI 全绿
- [x] CHANGELOG 已更新
- [ ] ROADMAP 同步（CBOR→JSON、R44 标记）
- [ ] coverage 基线（本地 `cargo llvm-cov`）

**S05 目标**：安全审计 — `cargo audit` 强化，E2E 加密审计，依赖漏洞闭环。
