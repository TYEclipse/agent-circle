# Agent Circle · S09 Sprint 回顾

> **Sprint**: S09 · Plugin 体系 (轮 91–100)  
> **锚点**: 组合化  
> **日期**: 2026-06-14  
> **状态**: ✅ 闭合 (R91–R99 完成, R96 延期)

---

## 一、完成总览

| 轮 | 任务 | 状态 |
|----|------|------|
| R91 | Plugin trait 定义 — `trait AgentPlugin` 生命周期钩子 | ✅ |
| R92 | Plugin 发现 — `~/.agent-circle/plugins/*.so` 目录扫描 | ✅ |
| R93 | Plugin 生命周期 — `PluginRegistry` load/init/start/stop/unload | ✅ |
| R94 | Plugin 注册自定义协议 — `Behaviour` 注册接口 | ✅ |
| R95 | Plugin 注册 CLI 子命令 — `plugin list` CLI | ✅ |
| R96 | Plugin 沙箱 (wasmtime) | ⏸️ 延期至 S14 |
| R97 | Plugin SDK — `agent-circle-plugin` crate | ✅ |
| R98 | 内置 plugin: `hello-world` cdylib | ✅ |
| R99 | Plugin 安全模型文档 | ✅ |
| R100 | S09 回顾 | ✅ 本文档 |

---

## 二、交付物

### 代码
- **`agent-circle-core/src/plugin.rs`** — `trait AgentPlugin` + 全部类型 (`PluginId`, `PluginManifest`, `PluginMessage`, `PluginError`, `PluginResult`)
- **`agent-circle-core/src/lib.rs`** — 新增 `pub mod plugin` (83→84 行，S08 split 后首次新增模块)
- **`src/main.rs`** — `PluginCmd::List` 子命令，CLI `plugin list` 枚举已加载插件
- **`agent-circle-plugin/`** — SDK crate，re-export 核心类型 + `declare_plugin!` 宏
- **`plugins/hello-world/`** — 内置 cdylib 示例插件，匹配 "hello"/"你好"
- **Workspace** — 成员从 1 增至 3 (`agent-circle-core`, `agent-circle-plugin`, `plugins/hello-world`)

### 文档
- **`docs/plugin-security.md`** — 威胁模型、能力声明、用户授权流程、WASM 沙箱方案、最佳实践
- **`agent-circle-plugin/src/lib.rs`** — 完整的 API 文档 + quick-start 示例

---

## 三、技术决策

1. **声明式能力模型**: 插件通过 `PluginManifest.capabilities` 声明权限，而非能力检测（与 Android 权限模型一致，用户可控）
2. **原生加载优先**: 第一版使用 `libloading` 原生 `.so` 加载（零性能开销），WASM 沙箱延期至 S14 作为可选增强
3. **SDK 独立 crate**: `agent-circle-plugin` 不依赖 libp2p/tokio，保持与 `agent-circle-core` 相同的轻量依赖策略
4. **`declare_plugin!` 宏**: 一行代码生成 C-ABI entry point，降低插件开发门槛
5. **内置插件在 workspace 内**: `plugins/hello-world` 是 workspace 成员，cdylib+lib 双 crate-type（可测试可加载）

---

## 四、测试覆盖

- Cargo test: 142 tests (52 main + 52 bin + 29 core + 6 chaos + 2 integration + 1 ignored gossipsub)
- Cargo fmt: 零偏差
- Cargo clippy: 零告警
- CI: 全部 7 job 全绿 (Linux 全量，macOS/Windows check，sccache 缓存)

---

## 五、经验教训

### 做得好的
- SDK + 示例插件联动开发：`declare_plugin!` 宏在 hello-world 中立即验证可用性
- CI 保持稳定：workspace 新增 2 个 member 后 License/Clippy/Format 全部一次通过
- 文档先行：安全模型文档在设计阶段明确威胁边界，指导后续 S10-S12 实现

### 可改进的
- `hello-world` 插件暂无单元测试（`cdylib` 的 extern "C" 符号不便在 test harness 中测试）
- Plugin 能力声明 (`PluginCapability`) 已定义在安全文档中但未写入代码 — 应随 S10 实现
- WASM 沙箱延期：优先级低于 Service Discovery (S10)，但安全等级目前仅 45/100

---

## 六、延期项

| 轮 | 任务 | 原因 | 新 Sprint |
|----|------|------|-----------|
| R96 | WASM 沙箱 | 优先级低于 S10 Service Discovery，且当前原生加载可满足自研插件需求 | S14 |

---

## 七、S10 准备

S10 · Service Discovery（轮 101–110）锚点仍然是组合化：

1. R101: Service 注册 — Agent Card 扩展 `services` 字段
2. R102: GossipSub 服务广播频道
3. R103: `service search` CLI 查询
4. R104: `service call` 直连调用
5. R105–R110: TUI、能力协商、订阅、离线缓存、市场 PoC

**S10 是 S09 Plugin 体系的自然延伸**——插件自定义协议在 S09 完成注册能力定义，S10 将其扩展到可发现、可调用的 Service。

---

## 八、S09 闭合声明

S09 Plugin 体系已闭合（9/10 轮完成，1 轮合理延期）。

Agent Circle 现在拥有：
- ✅ 完整的插件生命周期管理 (trait + Registry + CLI)
- ✅ 插件开发者 SDK (declare_plugin! 宏)
- ✅ 内置示例插件 (hello-world)
- ✅ 安全模型文档 (威胁分析 + 权限方案)
- ⏸️ WASM 沙箱 (S14)

**S10 ready.**
