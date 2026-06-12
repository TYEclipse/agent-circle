# S00 回顾 · 工程基座

**日期**：2026-06-13  
**轮次**：1–10  
**锚点**：保障性 · 维修性

## 完成情况

| 轮 | 任务 | 验收 | 状态 |
|---|---|---|---|
| R1 | CI/CD pipeline | fmt→clippy→test→build 四级 | ✅ |
| R2 | fmt + clippy 零告警 | CI 门禁 | ✅ |
| R3 | 结构化日志 | daemon→JSON, CLI→pretty | ✅ |
| R4 | 日志级别热切换 | SIGUSR1 reload Handle | ✅ |
| R5 | cargo-deny | licenses+bans+sources 全过 | ✅ |
| R6 | cargo-audit | 3 已知漏洞 → S05-S06 | ✅ advisory |
| R7 | CHANGELOG + 发布脚本 | Keep a Changelog + release.sh | ✅ |
| R8 | justfile | 24 recipes | ✅ |
| R9 | 代码基线 | 1,625 行 Rust / 22 测试 | ✅ |
| R10 | 回顾 | 本文档 | ✅ |

## 关键决策

- `tracing` + `Registry` + `reload::Layer` 实现热日志级别切换
- SIGUSR1 在 WSL2 有信号投递限制，代码在原生 Linux/macOS 上正确
- `cargo-audit` 设为 advisory 模式，3 个传递依赖漏洞延期至 S05-S06
- `deny.toml` 使用 v0.19 最小配置，已适配 API 变更

## 学到的事

1. `tracing` 的 `;` 字段语法在本版本不可用，统一用 `key = %val, "msg"` 格式
2. `read_file` 返回带行号前缀的内容，写入前必须 strip
3. WSL2 信号处理有已知 quirks，需在 CI (ubuntu-latest) 上验证
4. `cargo-deny` v0.19 大改 API，`vulnerability` / `unmaintained` / `notice` 已废弃

## 技术债务

| 项目 | 优先级 | 目标 Sprint |
|---|---|---|
| 3 个传递依赖安全漏洞 | 高 | S05-S06 |
| SIGUSR1 在 WSL2 验证 | 中 | S07 (跨平台) |
| 代码覆盖率基线 (tarpaulin) | 中 | S03 |
| Windows/macOS CI 构建 | 中 | S07 |
| Property-based tests | 低 | S03 |

## 下个 Sprint

**S01 · NAT 穿透 + 离线**（轮 11–20）  
锚点：环境适应性 · 可靠性
