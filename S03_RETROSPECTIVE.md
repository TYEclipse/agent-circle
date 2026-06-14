# S03 Sprint 回顾 — 测试体系建设

**日期**：2026-06-14  
**轮次**：R31–R40  
**Sprint 目标**：测试性锚点 → 行覆盖率 >80%

---

## 完成情况

| 轮 | 任务 | 状态 | 测试增量 |
|----|------|------|----------|
| R31 | 覆盖率基线 | ✅ | — |
| R32 | identity.rs → 100% | ✅ | +9 tests |
| R33 | storage.rs → 100% | ✅ | +11 tests |
| R34 | protocol.rs → 100% | ⏭️ 跳过 | protocol.rs 无独立模块 |
| R35 | chat.rs → 100% | ✅ | +9 tests |
| R36 | timeline.rs → 100% | ✅ | +15 tests |
| R37 | network 集成测试 | ✅ | +2 tests |
| R38 | Mock swarm 框架 | ✅ | MockNode |
| R39 | 测试数据工厂 | ✅ | 14 fixtures |
| R40 | S03 回顾 | ✅ | 本文档 |

**交付**：76 单元测试 + 2 集成测试，线上 CI 全绿 (<60s)，clippy 零告警。

---

## 覆盖率

| 模块 | 行数 | 覆盖 | 状态 |
|------|------|------|------|
| chat.rs | 40 | 100% | ✅ |
| dedup.rs | ~60 | 20% | ⚠️ 延期 S04 |
| diag.rs | ~90 | 0% | ⚠️ 延期 S04 |
| errors.rs | ~15 | 0% | ⚠️ (类型定义，无需覆盖) |
| identity.rs | 200+ | 100% | ✅ |
| message_queue.rs | ~200 | 0% | ⚠️ 延期 S04 |
| network.rs | ~300 | 0% | ⚠️ 延期 S04 |
| reliability.rs | ~100 | 0% | ⚠️ 延期 S04 |
| sequence.rs | ~80 | 0% | ⚠️ 延期 S04 |
| storage.rs | 100+ | 100% | ✅ |
| timeline.rs | 200+ | 100% | ✅ |

> 注：dedup/reliability/sequence/message_queue/network 模块有 S02 的集成测试验证（MockNode + 端到端投递），但缺少独立单元测试。这属于 S04 Fuzz + S05 继续补测试的工作。

---

## 经验教训

### 做得好的
1. **先建 infra 再补测试**：MockNode + Fixtures 两步走，先搭好框架再填充测试，避免了重复写样板代码。
2. **内联测试优于独立测试文件**：Rust `#[cfg(test)] mod tests` 放在源文件底部，保持了测试与实现的就近性。
3. **error-path 覆盖优先**：identity.rs 首先覆盖了 decode_did_key 的所有错误分支，这些是传统 happy-path 测试最容易漏掉的。

### 待改进
1. **R34 protocol.rs 不存在**：ROADMAP 预设了 `protocol.rs` 模块，但实际上 handshake 逻辑分布在 network.rs 和 identity.rs 中。ROADMAP 应更新以反映实际模块结构。
2. **coverage 工具链慢**：`cargo llvm-cov` 在 CI 上耗时 >5min，不适合轻量 CI。本地跑即可。
3. **dedup/reliability/sequence 单测缺失**：这些模块属于 S02 交付，当时通过 e2e 集成测试验证行为正确，但缺少独立单元测试。需 S04–S05 补上。

---

## 风险与延期

| 风险 | 等级 | 处置 |
|------|------|------|
| protocol.rs 模块不存在 | 🟡 低 | 跳过 R34，handshake 测试随 S09 插件体系一起补 |
| coverage 工具慢 (>5min) | 🟡 低 | 覆盖率报告仅本地执行，不入 CI |
| diag/network 模块覆盖率为 0% | 🟠 中 | S04 Fuzz + S05 补齐 |

---

## 进入 S04 的准备

- [x] 所有交付代码已 push (`7859e53`)
- [x] CI 全绿
- [x] CHANGELOG 已更新
- [ ] ROADMAP 更新 R34 状态
- [ ] coverage 基线数据填回

**S04 目标**：Fuzz + 混沌工程 — `cargo fuzz` 集成，反序列化/解析/DID 模糊测试。
