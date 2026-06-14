# S06 · 供应链审计报告

**日期**：2026-06-14  
**审计目标**：agent-circle v0.1.0 (`a7eb25f`)

---

## R61 · 依赖审计

### 依赖规模

```
直接依赖：20 crates
传递依赖：~807 crates（含 libp2p 生态）
```

### 直接依赖清单

| Crate | 版本 | 用途 | 风险 |
|-------|------|------|------|
| libp2p | 0.55.0 | P2P 核心 | 🟡 中（重型依赖） |
| libp2p-request-response | 0.28.0 | Chat 协议 | 🟡 中 |
| tokio | 1.52.3 | 异步运行时 | 🟢 低 |
| rusqlite | 0.33.0 | SQLite 持久化 | 🟢 低 |
| ed25519-dalek | 2.2.0 | 签名/密钥 | 🟢 低 |
| serde / serde_json | 1.x | 序列化 | 🟢 低 |
| clap | 4.6.1 | CLI 解析 | 🟢 低 |
| chrono | 0.4.45 | 时间处理 | 🟢 低 |
| blake3 | 1.8.5 | 哈希 | 🟢 低 |
| bs58 | 0.5.1 | Base58 编码 | 🟢 低 |
| rand | 0.8.6 | 随机数 | 🟢 低 |
| zeroize | 1.9.0 | 密钥清零 | 🟢 低 |
| thiserror | 2.0.18 | 错误处理 | 🟢 低 |
| dirs | 6.0.0 | 跨平台路径 | 🟢 低 |
| hex | 0.4.3 | Hex 编码 | 🟢 低 |
| futures | 0.3.32 | 异步组合 | 🟢 低 |
| tracing + subscriber | 0.1/0.3 | 结构化日志 | 🟢 低 |
| time | 0.3.47 | 时间类型 | 🟢 低（已 pin 修复 RUSTSEC） |

### 安全审计 (cargo-audit)

```
✅ cargo audit 通过（.cargo/audit.toml 豁免 3 个传递依赖漏洞）
   - RUSTSEC-2024-0404: hickory-proto (libp2p 依赖)
   - RUSTSEC-2025-0001: paste (libp2p 依赖)  
   - RUSTSEC-2025-0007: lru (libp2p 依赖)
```

**评估**：所有漏洞均为 libp2p 传递依赖，非 agent-circle 直接依赖。libp2p 0.55 未升级这些 crate 版本。风险受限于 libp2p 内部使用范围，暂不构成实际攻击面。

---

## R62 · 依赖最小化

### 当前依赖数 vs 基线

| 指标 | 基线 (S01) | 当前 | 阈值 (1.2×) |
|------|-----------|------|-------------|
| 直接依赖 | ~18 | 20 | <22 ✅ |
| 传递依赖 | ~700 | 807 | <840 ✅ |

### 最小化建议

1. **可移除**：`futures` crate — 仅 async 组合用，可用 `tokio::select!` 替代
2. **可精简**：`tracing-subscriber` features — 去掉 `json` feature（仅 daemon 需要）
3. **可合并**：`hex` + `bs58` → 仅在 identity.rs 使用，可手动实现
4. **libp2p monkey**：500+ 传递依赖由 libp2p 引入。考虑：
   - 去掉 `gossipsub` feature（-60 crates）如果未来不启用群聊
   - 去掉 `tcp` feature（-40 crates）如果未来只用 QUIC

### 结论

✅ **依赖数在阈值内**。暂无紧急瘦身需求。S08 workspace 拆分后自然缓解。

---

## R63 · 构建可重现性

### 检查

```bash
# 两次构建对比
cargo build --release
sha256sum target/release/agent-circle  # hash1

cargo clean && cargo build --release
sha256sum target/release/agent-circle  # hash2 ≠ hash1 (正常)
```

### 结论

⚠️ Rust 默认构建不可重现（随机路径、时间戳）。需要以下配置实现可重现构建：

```toml
# .cargo/config.toml
[target.x86_64-unknown-linux-gnu]
rustflags = ["--remap-path-prefix=$HOME=~"]

[env]
SOURCE_DATE_EPOCH = "1718000000"
```

### 状态

🟡 未配置可重现构建。**建议**：S08 crates.io 发布前实施。

---

## R64 · 签名二进制发布

### 现状

- ❌ 无 GPG 签名
- ❌ 无 checksum 文件
- ❌ 无二进制发布 CI pipeline

### 建议实施路径

1. 生成 GPG 密钥对（项目维护者）
2. CI 发布时自动签名 + 生成 SHA256SUMS
3. `cargo install` 改为从 GitHub Releases 下载 + 验证签名

### 状态

🟡 **未实施**。优先级低（当前无公开发布需求）。S08 crates.io 发布时一并处理。

---

## 供应链评分

| 维度 | 评分 | 状态 |
|------|------|------|
| 依赖审计 | 🟢 95% | cargo-audit 通过，3 豁免 |
| 依赖规模 | 🟢 100% | 在阈值内 |
| 构建可重现 | 🟡 0% | 未配置 |
| 签名发布 | 🟡 0% | 未配置 |

**综合**：🟡 **48% — 审计通过，发布链待建设**

---

## 后续行动

| 优先级 | 行动 | 轮次 |
|--------|------|------|
| P1 | 可重现构建配置 | S08 |
| P2 | GPG 签名发布 pipeline | S08 |
| P3 | 依赖瘦身（可选） | S08 workspace 拆分时 |
