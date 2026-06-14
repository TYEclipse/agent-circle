# S06 Sprint 回顾 — 供应链 + 密钥

**日期**：2026-06-14
**轮次**：R61–R70
**Sprint 目标**：安全性锚点（延续 S05）→ 供应链审计 + 密钥管理

---

## 完成情况

| 轮 | 任务 | 状态 | 交付 |
|----|------|------|------|
| R61 | 依赖审计 + 锁定 | ✅ | SUPPLY_CHAIN.md |
| R62 | 依赖最小化 | ✅ | 20 direct deps (阈值内) |
| R63 | 构建可重现性 | ⚠️ 记录 | 待 S08 实施 |
| R64 | 签名二进制发布 | ⚠️ 记录 | 待 S08 实施 |
| R65 | 密钥派生 (BIP-39) | ✅ | keys.rs — mnemonic → Ed25519 |
| R66 | 密钥备份/恢复 | ✅ | `identity mnemonic` / `identity restore` CLI |
| R67 | 多密钥 Profile | ⏭️ | 延期 S08 |
| R68 | 密钥轮换协议 | ⏭️ | 延期 S09 |
| R69 | 硬件密钥 | ⏭️ | 延期 S10+ |
| R70 | S06 回顾 | ✅ | 本文档 |

**交付**：89 tests (+5 keys)，SUPPLY_CHAIN.md，BIP-39 CLI

---

## 新增能力

```
agent-circle identity mnemonic          # 生成 12 词 BIP-39 助记词
agent-circle identity restore "<words>" # 从助记词恢复身份
agent-circle identity restore "<words>" -p "secret"  # 带密码短语
```

---

## 经验教训

### 做得好的
1. **BIP-39 集成简洁**：`bip39` crate 一行完成 PBKDF2→seed 派生，5 个测试覆盖正确性
2. **供应链审计文档化**：807 传递依赖可视化，3 个豁免有据可查

### 待改进
1. **R67–R69 范围过大**：多 Profile、密钥轮换、硬件密钥各需独立 sprint。ROADMAP 低估了工作量
2. **可重现构建未实施**：需 `SOURCE_DATE_EPOCH` + `--remap-path-prefix`，文档已记录但未执行

---

## 进入 S07 的准备

- [x] 交付 push (`7ece95c`)
- [x] CI 全绿 (89 tests)
- [x] CHANGELOG 更新

**S07 目标**：跨平台构建 — Linux/macOS/Windows CI
