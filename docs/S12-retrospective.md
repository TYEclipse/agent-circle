# S12 回顾 · 文档 + 打包

**Sprint**: S12 · 轮 121–130
**锚点**: 保障性（一键部署 + CI/CD）
**日期**: 2026-06-14

## 完成轮次

| 轮 | 任务 | 状态 |
|----|------|------|
| 121 | 用户手册 `docs/user-guide.md` | ✅ 11章，从安装到发朋友圈 |
| 122 | API 文档 `docs/api/` | ✅ 7 模块 ~925 行 |
| 123 | 协议规范 `docs/protocol-spec.md` | ✅ 10 章 wire format |
| 124 | 贡献指南 `CONTRIBUTING.md` | ✅ 环境·规范·PR·测试·发布 |
| 125 | `cargo install` 一键安装 | ✅ Git 安装 + crates.io 元数据准备 |
| 126 | `.deb` 打包 | ✅ 4.4 MB amd64 + systemd unit |
| 127 | `.rpm` 打包 | ✅ 7.1 MB x86_64 (Python builder) |
| 128 | Homebrew formula | ✅ Ruby formula + launchd plist |
| 129 | Docker 镜像 | ✅ 多阶段 Dockerfile (GFW 限本地构建) |
| 130 | S12 回顾 | ✅ 本文档 |

## 关键决策

1. **crates.io 未发布**: `agent-circle-core` 先发布才能发布主 crate；Git 安装 (`cargo install --git`) 作为主通道可用。
2. **RPM 构建**: `cargo-rpm` 不支持 workspace 版本继承，`cargo-generate-rpm` 依赖过重；自写 70 行 Python builder 解决。
3. **Docker GFW 阻塞**: Docker Hub 286MB 镜像在 GFW 下极慢，Dockerfile 已就绪，CI/GitHub Actions 可构建。
4. **systemd user unit**: 使用用户级 systemd（非系统级），适合每用户运行 P2P agent 的场景。

## 度量

- **新增文件**: 8 个 (user-guide.md, 7 API docs, protocol-spec.md, CONTRIBUTING.md, LICENSE, Dockerfile, .dockerignore, 4 packaging files)
- **修改文件**: Cargo.toml (deb+rpm 元数据), ROADMAP.md
- **总文档行数**: ~3,500+ 行
- **打包格式**: 5 种 (Git install / .deb / .rpm / Homebrew / Docker)

## 遗留

- Docker 镜像待 CI 验证构建
- crates.io 发布待 `agent-circle-core` 先行
- 协议跟踪日志 (R114) 推至 S12 但未执行 → 重新列入 S19

## 进度

S12 = 10/10 ✅ · 总进度 = 130/200 (65%)
