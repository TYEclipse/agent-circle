# Agent Circle · Plugin 安全模型

---

## 1. 威胁模型 (Threat Model)

Agent Circle 插件是编译为 `.so` 的动态库，通过 `libloading` 在 daemon 进程内加载。
这意味着插件拥有与 daemon 进程**完全相同的权限**——内存访问、文件系统、网络、系统调用。

### 1.1 攻击面

| 攻击面 | 风险 | 缓解 |
|--------|------|------|
| 恶意 `.so` 注入插件目录 | 进程接管、数据窃取 | 文件系统权限 + 签名验证 (计划) |
| 良性插件漏洞被利用 (RCE) | 通过插件代码路径执行任意指令 | WASM sandbox (可选) |
| 插件读取敏感文件 (私钥、联系人) | 身份泄露 | 能力声明 + 用户授权 |
| 插件发起未授权网络连接 | C2 回连、数据外泄 | 能力声明 + 网络 eBPF (计划) |
| 插件注册恶意 Behaviour | 中间人、消息窃听 | Behaviour 注册白名单审核 |
| 插件 panic → 进程崩溃 | DoS | `catch_unwind` wrapper + 隔离插件 |
| 插件版本冲突 (符号 hijack) | 未定义行为 | 独立 `.so` + symbol 隔离 |

### 1.2 信任层级

```
完全不信任 (第三方市场)  →  WASM sandbox (计划)
│
社区插件 (GitHub 开源)   →  代码审计 + 签名
│
自研插件 (内部开发)      →  原生加载 (当前默认)
```

---

## 2. 权限模型 — 能力声明 (Capability Declaration)

插件必须在 `PluginManifest` 中声明所需权限，daemon 在加载时校验。

### 2.1 能力定义

```rust
/// 插件请求的系统能力
pub enum PluginCapability {
    /// 读取聊天消息内容
    ReadMessages,
    /// 发送消息（代表用户）
    SendMessages,
    /// 读取本地存储 (Agent Card / Contacts / Timeline)
    ReadStorage,
    /// 写入本地存储
    WriteStorage,
    /// 注册自定义协议 Behaviour
    RegisterProtocol,
    /// 注册 CLI 子命令
    RegisterCli,
    /// 访问网络 (额外出口连接)
    NetworkAccess,
    /// 访问文件系统 (插件目录之外)
    FileSystemAccess,
}
```

### 2.2 Manifest 扩展

```rust
pub struct PluginManifest {
    // ... 现有字段 ...
    /// 插件需要的能力。daemon 按此授权。
    pub capabilities: Vec<PluginCapability>,
    /// 插件签名（Ed25519, base58 编码），验证后加载
    pub signature: Option<String>,
    /// 签名者的公钥 (did:key)
    pub signer: Option<String>,
}
```

---

## 3. 用户授权流程

### 3.1 首次加载

```
1. daemon 扫描 ~/.agent-circle/plugins/
2. 对每个 .so 提取 PluginManifest (dlopen + dlsym manifest)
3. 校验 Ed25519 签名（如存在）
4. 展示能力清单给用户：
   ┌─────────────────────────────────────────┐
   │ Plugin:  weather-bot v0.2.0             │
   │ Author:  weather-team@example.com       │
   │ Signer:  did:key:z6Mk...                │
   │                                         │
   │ 请求权限:                                │
   │   ✅ ReadMessages    (读取聊天)          │
   │   ✅ SendMessages    (发送消息)          │
   │   ❌ ReadStorage                        │
   │   ❌ WriteStorage                       │
   │   ✅ NetworkAccess   (查询天气 API)       │
   │                                         │
   │ [授权并加载]  [拒绝]  [仅本次]  [永久信任] │
   └─────────────────────────────────────────┘
5. 用户选择 → 授权记录写入 ~/.agent-circle/grants.json
6. daemon 调用 on_init() → on_start()
```

### 3.2 grants.json 格式

```json
{
  "granted": {
    "weather-bot": {
      "so_path": "~/.agent-circle/plugins/libweather_bot.so",
      "so_hash": "sha256:abcd1234...",
      "capabilities_granted": ["ReadMessages", "SendMessages", "NetworkAccess"],
      "signer": "did:key:z6Mk...",
      "granted_at": "2026-06-14T12:00:00Z",
      "expires": null
    }
  }
}
```

---

## 4. 沙箱方案

### 4.1 当前方案：原生加载 (Native)

插件作为 `.so` 被 `dlopen` 加载到 daemon 进程。能力校验通过在 trait 方法调用前做运行时检查：

```rust
fn on_chat_message(msg: &PluginMessage) {
    if !registry.has_capability(&plugin_id, PluginCapability::ReadMessages) {
        return Err(PluginError::capability_denied("ReadMessages"));
    }
    // ... 实际处理
}
```

**优点**: 零性能开销、开发简单  
**缺点**: 无内存隔离、恶意插件可绕过 trait 接口

### 4.2 计划方案：WASM 沙箱 (可选 R96)

使用 `wasmtime` 将插件编译为 `.wasm`，在隔离的 WASM 实例中运行：

| 维度 | 原生加载 | WASM 沙箱 |
|------|---------|----------|
| 内存隔离 | ❌ | ✅ |
| 文件系统控制 | ❌ (OS 级) | ✅ (WASI 目录映射) |
| 网络控制 | ❌ | ✅ (WASI socket allowlist) |
| 性能 | 100% | ~80–95% |
| 开发体验 | Rust 全能力 | 受限 WASM 环境 |
| 适用场景 | 内部/自研插件 | 第三方/市场插件 |

### 4.3 迁移路径

- 第一阶段 (当前): 原生加载 + 能力声明 + 用户授权
- 第二阶段: WASM 后端作为可选 feature flag (`--sandbox wasm`)
- 第三阶段: 第三方插件默认 WASM，内部插件可选原生

---

## 5. 插件开发者安全最佳实践

### 5.1 声明最小权限

```rust
// ✅ 好：只声明真正需要的能力
fn manifest(&self) -> PluginManifest {
    PluginManifest {
        // ...
        capabilities: vec![PluginCapability::ReadMessages],
    }
}

// ❌ 坏：请求所有权限 "以防万一"
```

### 5.2 验证输入

```rust
fn on_chat_message(&mut self, msg: &PluginMessage) -> PluginResult<bool> {
    // ✅ 验证 peer_id 格式
    if msg.peer_id.len() > 256 { return Ok(false); }

    // ✅ 限制内容长度，防止 OOM
    if msg.content.len() > 10_000 { return Ok(false); }

    Ok(msg.content.contains("hello"))
}
```

### 5.3 不要 panic

```rust
// ❌ 插件 panic 会杀死整个 daemon
fn on_chat_message(&mut self, msg: &PluginMessage) -> PluginResult<bool> {
    let x = msg.content.parse::<u64>().unwrap(); // 💥
    Ok(true)
}

// ✅ 用 Result 优雅降级
fn on_chat_message(&mut self, msg: &PluginMessage) -> PluginResult<bool> {
    let _ = msg.content.parse::<u64>().map(|x| x * 2);
    Ok(false)
}
```

### 5.4 签名发布

```bash
# 生成插件签名密钥
agent-circle identity generate --name "plugin-signer"

# 签名插件
agent-circle plugin sign libmy_plugin.so --key ~/.agent-circle/identity.json

# 发布时附带签名，用户 daemon 自动验证
```

---

## 6. 当前安全等级

| 维度 | 评分 | 说明 |
|------|------|------|
| 代码隔离 | 20% | 原生加载，无内存隔离 |
| 权限控制 | 50% | 能力声明定义完成，运行时 enforce 待实现 |
| 用户授权 | 40% | grants.json 格式定义，UI 交互待实现 |
| 签名验证 | 30% | Manifest 字段定义，签名验证逻辑待实现 |
| Panic 隔离 | 60% | `catch_unwind` 可在 PluginRegistry 包装 |
| 依赖安全 | 80% | `cargo deny` + `cargo audit` 覆盖 |

**综合评分：45/100** — 处于"设计了安全模型，等待实现"阶段。S10–S12 逐步实现。

---

## 7. 后续迭代

| Sprint | 安全增强 |
|--------|----------|
| S10 | `PluginCapability` 运行时 enforce |
| S11 | `grants.json` 用户授权交互 (CLI/TUI) |
| S12 | Ed25519 签名验证 |
| S14 | WASM sandbox 集成 (wasmtime) |
| S19 | v1.0 最终安全审计 |
