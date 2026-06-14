# Contributing to Agent Circle

感谢你对 Agent Circle P2P Agent 社交基础设施的关注！本文件帮助你从 clone 代码到提交 PR 的每一步。

## 开发环境

### 前提条件

| 工具 | 版本 | 用途 |
|------|------|------|
| **Rust** | 1.96+ | 编译器 + 工具链 (rustc, cargo, rustup) |
| **just** | 1.x | 任务编排 (`cargo install just`) |
| **cargo-deny** | 0.17+ | 许可证检查 (`cargo install cargo-deny`) |
| **cargo-audit** | 可选 | 漏洞扫描 (`cargo install cargo-audit`) |
| **cargo-tarpaulin** | 可选 | 代码覆盖率 (`cargo install cargo-tarpaulin`) |
| **tokei** | 可选 | 代码统计 (`cargo install tokei`) |

### 中国镜像加速

如果你的网络受限，可使用中科大镜像加速 Rust 工具链：

```bash
# ~/.cargo/config.toml
[source.crates-io]
replace-with = 'ustc'

[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
```

### 克隆仓库

```bash
git clone git@github.com:TYEclipse/agent-circle.git
cd agent-circle
```

## 项目结构

```
agent-circle/
├── src/                    # 主 binary (agent-circle CLI)
│   ├── main.rs             # CLI 入口 + 所有子命令
│   ├── network.rs          # libp2p swarm 管理
│   ├── storage.rs          # 磁盘 I/O (身份/联系人/时间线)
│   ├── message_queue.rs    # 离线消息队列 + ACK 追踪
│   ├── service_discovery.rs # 服务注册/发现
│   ├── metrics.rs          # OpenMetrics 指标导出
│   ├── health.rs           # HTTP 健康检查端点
│   └── crash.rs            # Panic dump 系统
├── agent-circle-core/      # 核心库 (数据结构 + 协议类型)
│   └── src/
│       ├── identity.rs     # Identity, AgentCard, keys
│       ├── error.rs        # AcError 统一错误码
│       └── lib.rs
├── agent-circle-plugin/    # 插件 SDK
├── plugins/                # 内置插件
│   └── hello-world/
├── fuzz/                   # Fuzz 测试目标
├── tests/                  # 集成测试
│   └── common/fixtures.rs
├── docs/                   # 文档
│   ├── api/                # API 参考
│   ├── user-guide.md
│   ├── protocol-spec.md
├── justfile                # 任务编排
├── Cargo.toml              # Workspace 清单
├── CHANGELOG.md
├── README.md
```

## 构建与测试

### 快速开始

```bash
just build        # 编译 debug
just test         # 运行全部测试
just ci           # 全量 CI：格式 + lint + 测试
```

### 质量门禁（提交前必须通过）

提交 PR 前，本地必须通过以下三项：

```bash
just fmt-check    # cargo fmt --all -- --check 零偏差
just lint         # cargo clippy --all-targets --all-features -- -D warnings 零告警
just test         # cargo test --all-targets 全部通过
```

**有一条不通过，不要提交。**

### 其他常用命令

| 命令 | 用途 |
|------|------|
| `just test-integration` | 只跑带 `#[ignore]` 的集成测试 |
| `just test-verbose` | 带输出的测试 |
| `just fix` | 自动修复 clippy 告警 |
| `just deny` | 许可证审计 |
| `just audit` | 安全漏洞扫描 |
| `just coverage` | 代码覆盖率报告 → `target/coverage/` |
| `just loc` | 代码行数统计 |
| `just watch` | 文件变动自动跑测试 (`cargo watch`) |

## 代码规范

### Rust 风格

- 严格遵循 `rustfmt` 默认配置
- 禁止 `unwrap()` / `expect()` 在生产路径中使用 — 使用 `?` 或 `match`
- 所有 `pub` 项必须有文档注释 (`///` 或 `//!`)
- 新增公共逻辑必须有单元测试
- 错误类型统一使用 `agent-circle-core` 的 `AcError` 枚举（非字符串错误）

### 提交信息规范

格式：`feat: 简短英文标题 — 中文详情` 或 `fix: 简短描述`

| 示例 | 说明 |
|------|------|
| `feat: CONTRIBUTING.md — 完整的贡献指南` | 功能提交 |
| `feat: remote diagnosis — doctor --peer <PEER_ID>` | 带命令行参数 |

- 使用 Conventional Commits 前缀：`feat:` / `fix:` / `docs:` / `refactor:` / `test:`
- 英文标题简短概括改动（首字母大写，不加句号）
- `—` 后中文详情说明做了什么

### 分支策略

```
master  ←──  功能分支 (feature/xxx)
  ↑
  所有开发在 master 上直接提交或从 master 分支
```

本项目采用 **主干开发** 模式：所有改动直接或通过短生命分支合并到 `master`，依靠 CI 门禁保证质量。大特性可用 feature 分支，但必须及时合并。

### 向后兼容

- 公共 API（`agent-circle-core` 类型、CLI 子命令、协议消息格式）破坏性变更必须在 CHANGELOG 中标注 `**BREAKING**`
- 协议版本号遵循 SemVer：MAJOR.MINOR.PATCH
- 修改网络协议必须更新 `docs/protocol-spec.md`

## PR 流程

### 1. 开 Issue

每个开发工作对应一个功能规划条目或一个 GitHub Issue。如果做的是计划外的改动，先开 Issue 讨论。

### 2. 创建分支

```bash
git checkout -b feature/xxx
```

### 3. 开发 + 本地自检

```bash
# 写代码...

just ci                # 全量门禁
```

### 4. 提交

```bash
git add -A
git commit -m "feat: CONTRIBUTING.md — 完整的贡献指南"
```

### 5. 推送 + 开 PR

```bash
git push origin feature/xxx
# 在 GitHub 上创建 PR
```

### PR 模板检查清单

每个 PR 描述中必须包含：

```
- [ ] `cargo fmt --all -- --check` 通过
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` 通过
- [ ] `cargo test --all-targets` 全部通过
- [ ] CHANGELOG.md 已更新（如有用户可见改动）
- [ ] 公共 API 文档已更新（如有 API 改动）
```

### 6. CI 等待 + Review

GitHub Actions 自动运行 build + test + lint + fmt。等全部绿灯后请求 review。

## 测试指南

### 单元测试

放在 `src/` 或 `agent-circle-core/src/` 的同一文件底部：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specific_behavior() {
        // arrange
        // act
        // assert
    }
}
```

### 集成测试

放在 `tests/` 目录，依赖 `tests/common/fixtures.rs` 提供的公共 fixture。

```rust
use common::fixtures::create_test_identity;

#[tokio::test]
async fn test_end_to_end_flow() {
    let id = create_test_identity();
    // ...
}
```

### Fuzz 测试

放在 `fuzz/` 目录，使用 `cargo-fuzz`：

```bash
cd fuzz
cargo fuzz run <target_name>
```

## 发布流程

1. 更新 `CHANGELOG.md`（`Unreleased` → 版本号 + 日期）
2. 更新 `Cargo.toml` 版本号（所有 crate）
3. 提交：`git commit -m "release: vX.Y.Z"`
4. `just release X.Y.Z`（执行 `scripts/release.sh`）
5. 在 GitHub Releases 中创建 tag + release notes

## 获取帮助

- **用户文档**：`docs/user-guide.md`
- **协议规范**：`docs/protocol-spec.md`
- **API 参考**：`docs/api/`
- **问题讨论**：GitHub Issues

---

---
