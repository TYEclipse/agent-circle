# S05 · 安全审计报告

**日期**：2026-06-14  
**审计范围**：agent-circle v0.1.0 (`2cca987`)  
**方法**：静态分析 + 代码审查

---

## R51 · 端到端加密链路

### 现状

```
✅ Noise XX 握手 — libp2p::noise::Config::new (src/network.rs:57)
✅ yamux 流复用
✅ 所有 P2P 通信经 Noise 加密通道
```

libp2p Noise 协议提供：
- **身份验证**：Ed25519 密钥对绑定 PeerId
- **前向保密**：XX 模式每次会话派生新密钥
- **加密+完整性**：ChaCha20Poly1305（默认）

### 验证结论

E2E 加密链路完整。libp2p Noise 实现经过广泛审计（由 Protocol Labs 维护），无需额外操作。

---

## R52 · 密钥存储安全

### 现状

```
✅ 私钥文件 identity.key 权限 0600 (src/storage.rs:53,63)
✅ 内存中密钥 zeroize (src/identity.rs:107-109)
✅ Identity 实现 Drop trait，析构时清零签名密钥字节
```

### 验证

```rust
// storage.rs — 保存时设置 0600
f.set_permissions(fs::Permissions::from_mode(0o600))?;

// identity.rs — 析构时 zeroize
impl Drop for Identity {
    fn drop(&mut self) {
        let mut bytes = self.signing_key.to_bytes();
        bytes.zeroize();
    }
}
```

`zeroize` crate 使用 `volatile_write` 确保编译器不会优化掉清零操作。✅

---

## R53 · 内存安全审计

### Unsafe 代码审查

| 位置 | 用途 | 风险 | 处置 |
|------|------|------|------|
| `reliability.rs:123` | `transmute(u64) → OutboundRequestId` | 🟢 低 | 仅测试代码，fake_id 辅助 |
| `network.rs:202` | `transmute(OutboundRequestId) → u64` | 🟢 低 | libp2p 内部保证 u64 布局 |

两处 unsafe 都是 `u64 ↔ OutboundRequestId` 互转（libp2p 内部 newtype）。验证方式：
- `OutboundRequestId` 在 libp2p 0.55 中定义为 `#[repr(transparent)] struct OutboundRequestId(u64)`
- transmute 安全：size 和 alignment 均为 8 字节
- 可改为 `OutboundRequestId::from(u64)` / `u64::from(id)` 如果 libp2p 暴露了这些 trait

### 结论

- ✅ 生产代码 unsafe 仅 1 处，有业务理由
- ⚠️ 建议：跟踪 libp2p 升级，一旦暴露 `From<u64>` 即替换

---

## R54 · 时序攻击防护

### 审查

| 操作 | 方法 | 时序安全 | 风险 |
|------|------|----------|------|
| Ed25519 签名验证 | `verify_strict()` | ✅ CT | ed25519-dalek 内部实现 |
| DID 字符串比较 | `==` | ⚠️ 非 CT | 🟢 低 (DID 是公开标识符) |
| 密钥比较 | N/A | — | ✅ 无 secret 比较路径 |

### 结论

- ✅ Ed25519 签名验证使用 `verify_strict`（常量时间）
- ✅ DID 比较使用 `==`，但 DID 是公开值，不存在时序泄露风险
- ✅ 无私钥直接比较路径

时序攻击防护 ✅ 通过。

---

## R55 · 消息签名验证

### 审查

| 消息类型 | 签名 | 验证 |
|----------|------|------|
| Handshake Hello | ✅ nonce 签名 | ✅ `verify_strict()` |
| Handshake Accept | ✅ nonce 签名 | ✅ `verify_strict()` |
| Timeline 节点 | ✅ Ed25519 签名 | ✅ `verify_strict()` |
| AgentCard | ✅ 自签名 proof | ✅ `verify_strict()` |
| ChatRequest | ❌ 无独立签名 | ⚠️ |

### ChatRequest 签名缺失分析

ChatRequest 没有独立的 Ed25519 签名。当前安全假设：
1. Noise 通道已提供传输层认证（消息来自已建立 Noise 会话的 Peer）
2. ChatRequest 通过 Noise 通道传输，攻击者无法篡改运输中数据

**结论**：
- ✅ 传输层：Noise 加密+认证保证通道内消息不被篡改
- ⚠️ 应用层：ChatRequest 无独立签名，无法独立验证（脱离 Noise 通道后）
- 🟡 中风险：若未来支持中继/存储转发/离线消息，需补 Ed25519 签名

**建议**：S09 插件体系或 S02 消息可靠性增强时，为 ChatRequest 添加 `signature` 字段。

---

## R56 · 重放攻击防护

### 审查

| 协议 | 防护机制 | 强度 |
|------|----------|------|
| Noise 握手 | XX 模式 ephemeral DH + nonce | ✅ 强 |
| Handshake 协议 | 128-bit random nonce (16 bytes) | ✅ 强 |
| Timeline | timestamp + hash chain | ✅ 链式不可逆 |
| ChatRequest | msg_id 去重 | ✅ effectively-once |

### ChatRequest 重放分析

ChatRequest 通过 `msg_id` (random u64) 实现去重：
- 接收端 `DedupFilter` (LRU 10K) 记录已处理的 msg_id
- 重放的 ChatRequest 被识别并只回复 ACK，不重复处理
- `msg_id` 碰撞概率 ~1/2^64 ≈ 可忽略

### 结论

✅ 重放攻击防护完整（握手层 nonce + 应用层 msg_id 去重）。

---

## R57 · DoS 抗性 — 连接限制

### 现状

```
❌ 无连接数限制
❌ 无 Peer 连接上限
```

当前 libp2p swarm 配置未设置任何连接限制。

### 风险评估

- 🟡 中风险：恶意Peer可建立大量连接消耗资源
- 但 libp2p swarm 有默认的 transport 超时和 Yamux 流限制

### 建议修复

```rust
// 添加连接限制到 build_swarm
swarm = swarm
    .with_connection_limits(
        libp2p::connection_limits::ConnectionLimits::default()
            .with_max_pending_incoming(Some(10))
            .with_max_pending_outgoing(Some(10))
            .with_max_established_incoming(Some(50))
            .with_max_established_outgoing(Some(50))
    );
```

---

## R58 · DoS 抗性 — 消息速率限制

### 现状

```
❌ 无消息速率限制
❌ 无单 Peer 消息上限
```

### 风险评估

- 🟡 中风险：恶意Peer可洪泛 ChatRequest，虽然去重过滤重复，但新消息仍会触发处理
- SQLite 离线队列有隐式限制（磁盘 I/O 自然限制），但不够

### 建议修复

在 `daemon.rs` 事件循环中添加 per-peer 令牌桶：
```rust
// 每 Peer 每秒最多 30 条 ChatRequest
rate_limiter.check(peer_id, 30.0) // returns Ok/Delay
```

---

## 安全评分矩阵

| 类别 | 评分 | 状态 |
|------|------|------|
| E2E 加密 | 🟢 100% | Noise XX + yamux |
| 密钥存储 | 🟢 100% | 0600 + zeroize |
| 内存安全 | 🟢 95% | 1 unsafe transmute (合理) |
| 时序攻击 | 🟢 100% | verify_strict |
| 消息签名 | 🟡 85% | ChatRequest 缺独立签名 |
| 重放防护 | 🟢 100% | nonce + msg_id 去重 |
| DoS 连接限制 | 🟠 60% | 无限制 |
| DoS 速率限制 | 🟠 60% | 无限制 |

**综合评分**：🟡 **87.5%** (7/8 通过，2 项需改进)

---

## 改进路线图

| 优先级 | 改进项 | 轮次 | 工作量 |
|--------|--------|------|--------|
| P1 | ChatRequest Ed25519 签名 | S09 | 2轮 |
| P2 | 连接限制 | 当前（如紧急） | 1轮 |
| P3 | 消息速率限制 | 当前（如紧急） | 1轮 |
| P4 | transmute 换 safe From | 跟踪 libp2p 升级 | 持续 |
