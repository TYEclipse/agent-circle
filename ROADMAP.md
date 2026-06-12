# Agent Circle · 200 轮敏捷路线图

> **目标**：通过 200 轮敏捷迭代，将 agent-circle 从"能跑的原型"升级为符合三化六性标准的 P2P Agent 社交基础设施。

---

## 总览

| Sprint | 轮次 | 主题 | 三化六性锚点 |
|---|---|---|---|
| S00 | 1–10 | 工程基座 | 保障性 · 维修性 |
| S01 | 11–20 | NAT 穿透 + 离线 | 环境适应性 · 可靠性 |
| S02 | 21–30 | 消息可靠性 | 可靠性 |
| S03 | 31–40 | 测试体系建设 | 测试性 |
| S04 | 41–50 | Fuzz + 混沌 | 测试性 |
| S05 | 51–60 | 安全审计 | 安全性 |
| S06 | 61–70 | 供应链 + 密钥 | 安全性 |
| S07 | 71–80 | 跨平台构建 | 通用化 |
| S08 | 81–90 | 协议版本化 + crates | 系列化 |
| S09 | 91–100 | Plugin 体系 | 组合化 |
| S10 | 101–110 | Service Discovery | 组合化 |
| S11 | 111–120 | 诊断 + 日志 | 维修性 |
| S12 | 121–130 | 文档 + 打包 | 保障性 |
| S13 | 131–140 | 公众号（Agent 服务发布） | 功能补全 |
| S14 | 141–150 | 小程序（工具共享） | 功能补全 |
| S15 | 151–160 | TUI + UX 打磨 | 功能补全 |
| S16 | 161–170 | 压力测试 + 性能 | 可靠性 |
| S17 | 171–180 | 边界条件 + 长稳 | 可靠性 · 环境适应性 |
| S18 | 181–190 | 集成测试 + e2e | 测试性 |
| S19 | 191–200 | 最终审计 + v1.0 发布 | 九维验收 |

---

## S00 · 工程基座（轮 1–10）

**锚点**：保障性 · 维修性

| 轮 | 任务 | 验收 |
|---|---|---|
| 1 | CI/CD pipeline（GitHub Actions · build + test + lint） | PR 提交自动跑 |
| 2 | `cargo fmt` + `cargo clippy` 零告警 | CI 门禁 |
| 3 | 结构化日志（`tracing` + JSON 输出） | `RUST_LOG=debug agent-circle daemon` 输出 JSON 行 |
| 4 | 日志级别运行时动态切换 | `SIGUSR1` 切换 debug/info |
| 5 | `cargo deny` 许可证检查 | CI 集成 |
| 6 | `cargo audit` 漏洞扫描 | CI 集成，零严重漏洞 |
| 7 | CHANGELOG.md + 发布脚本 | `just release 0.3.0` 一键打 tag + 发布 |
| 8 | justfile 任务编排 | `just build`, `just test`, `just release` |
| 9 | 代码行数 + 复杂度基线报告 | CI 产出 `tokei` + `cognitive-complexity` 报告 |
| 10 | S00 回顾 + tech debt backlog | Sprint 回顾文档 |

---

## S01 · NAT 穿透 + 离线（轮 11–20）

**锚点**：环境适应性 · 可靠性

| 轮 | 任务 | 验收 |
|---|---|---|
| 11 | DCUtR hole-punching 验证 | 两个 NAT 后节点直连成功 |
| 12 | Relay 节点实现（最后一个 fallback） | NAT 类型对称时通过 relay 兜底 |
| 13 | Relay 发现协议（DHT 广播 relay 地址） | 新节点启动 5s 内找到 relay |
| 14 | 离线消息队列（SQLite 持久化） | 对方离线 → 消息入队 → 上线后自动推送 |
| 15 | 消息去重（dedup ID） | 重复消息自动丢弃 |
| 16 | 断线检测 + 指数退避重连 | 断线 < 3s 检测，退避 1s/2s/4s/8s/16s |
| 17 | 多 listen 地址支持（LAN + WAN 双监听） | `listen /ip4/0.0.0.0/tcp/0 /ip4/0.0.0.0/udp/0/quic-v1` |
| 18 | 低带宽模式（压缩 + 频率限制） | 带宽 < 1KB/s 时自适应降级 |
| 19 | 网络环境探测 + 自适应参数 | 启动时探测 RTT/带宽，调整心跳/重传参数 |
| 20 | S01 回顾 | Sprint 回顾文档 |

---

## S02 · 消息可靠性（轮 21–30）

**锚点**：可靠性（99.9% 消息投递）

| 轮 | 任务 | 验收 |
|---|---|---|
| 21 | 消息投递确认协议（ACK + NACK） | 发送 → ACK 或超时重传 |
| 22 | 消息序列号 + 顺序保证 | 乱序消息自动重排 |
| 23 | 消息重传队列（指数退避） | 超时未 ACK → 最多重传 5 次 |
| 24 | 投递状态回调（Delivered / Failed / Pending） | `chat send --track` 显示投递状态 |
| 25 | 消息可靠性压测工具 | 1000 条消息 → `count(ACK)/1000` |
| 26 | 99.9% 投递率验证（稳定网络） | 10000 条消息 → ≥ 9990 条成功 |
| 27 | 99% 投递率验证（丢包 5%） | 丢包 5% 环境 → ≥ 9900/10000 成功 |
| 28 | 消息去重 + 幂等投递 | 重传不导致重复投递到上层 |
| 29 | Crash recovery（重启后恢复未 ACK 队列） | kill -9 → 重启 → 自动重发未确认消息 |
| 30 | S02 回顾 | Sprint 回顾文档 |

---

## S03 · 测试体系建设（轮 31–40）

**锚点**：测试性（>80% 覆盖率）

| 轮 | 任务 | 验收 |
|---|---|---|
| 31 | `cargo tarpaulin` 行覆盖率基线 | CI 产出覆盖率报告 |
| 32 | `identity.rs` 覆盖率 → 100% | 100% 行覆盖 |
| 33 | `storage.rs` 覆盖率 → 100% | 100% 行覆盖 |
| 34 | `protocol.rs` 覆盖率 → 100% | 100% 行覆盖 |
| 35 | `chat.rs` 覆盖率 → >90% | >90% 行覆盖 |
| 36 | `timeline.rs` 覆盖率 → >90% | >90% 行覆盖 |
| 37 | `network.rs` 集成测试增强 | swarm 行为模拟测试 |
| 38 | Mock swarm 框架（轻量 libp2p mock） | 不需要真实网络即可测试协议逻辑 |
| 39 | 测试数据工厂（生成合法/非法消息） | `TestFixtures::random_agent_card()` |
| 40 | S03 回顾 | Sprint 回顾文档 |

---

## S04 · Fuzz + 混沌（轮 41–50）

**锚点**：测试性（fuzz）

| 轮 | 任务 | 验收 |
|---|---|---|
| 41 | `cargo fuzz` 集成 + CI | fuzz 在 CI 上持续运行 |
| 42 | CBOR 消息反序列化 fuzz | 任意字节 → 不 panic |
| 43 | DID 解析 fuzz | 任意字符串 → 不 panic |
| 44 | 握手协议消息 fuzz | 畸形握手消息 → 正确拒绝 |
| 45 | 时间线 Merkle-DAG fuzz | 随机 DAG 操作 → 不 panic |
| 46 | 网络消息注入 fuzz | 恶意 Peer 发送垃圾消息 → 不崩溃 |
| 47 | 混沌测试框架（随机断网/延迟/丢包） | `chaos-mesh` 或自定义 toxiproxy |
| 48 | 混沌测试 · 断网恢复 | 随机断网 5s → 恢复后消息不丢 |
| 49 | 混沌测试 · 节点重启 | 随机 kill → 重启 → 网络自愈 |
| 50 | S04 回顾 | Sprint 回顾文档 |

---

## S05 · 安全审计（轮 51–60）

**锚点**：安全性（E2E 审计）

| 轮 | 任务 | 验收 |
|---|---|---|
| 51 | 端到端加密链路审计 | Noise 握手 → 密钥派生 → 加密通道全链路验证 |
| 52 | 密钥存储安全审计 | 私钥文件权限 0600；内存中密钥 zeroize |
| 53 | 内存安全审计 | `unsafe` 代码零新增；现有 unsafe 逐一审查 |
| 54 | 时序攻击防护 | 密钥比较使用 constant-time |
| 55 | 消息签名验证 | 每条消息 Ed25519 签名 + 接收端验证 |
| 56 | 重放攻击防护（timestamp + nonce） | 重放 30s 外消息被拒绝 |
| 57 | DoS 抗性 · 连接限制 | 同一 Peer 最大连接数限制 |
| 58 | DoS 抗性 · 消息速率限制 | 同一 Peer 每秒消息上限 |
| 59 | 安全审计报告 | 完整 security audit 文档 |
| 60 | S05 回顾 | Sprint 回顾文档 |

---

## S06 · 供应链 + 密钥（轮 61–70）

**锚点**：安全性

| 轮 | 任务 | 验收 |
|---|---|---|
| 61 | 依赖审计 + 锁定 | `cargo vet` 或 `cargo crev` |
| 62 | 依赖最小化（去除非必要 crate） | 依赖数 < 基线 × 1.2 |
| 63 | 构建可重现性 | 同一 commit 两次 `cargo build` → 相同 hash |
| 64 | 签名二进制发布 | `cargo install` 下载的文件有 GPG 签名 |
| 65 | 密钥派生（BIP-39 助记词） | 12 词助记词 → Ed25519 密钥对 |
| 66 | 密钥备份/恢复 | `identity export --mnemonic` / `identity restore` |
| 67 | 多密钥 Profile 支持 | `~/.agent-circle/profiles/<name>/identity.key` |
| 68 | 密钥轮换协议 | 旧密钥签名 → 新密钥广播 → 联系人自动更新 |
| 69 | 硬件密钥支持（可选 YubiKey） | Linux 下读取 YubiKey Ed25519 |
| 70 | S06 回顾 | Sprint 回顾文档 |

---

## S07 · 跨平台构建（轮 71–80）

**锚点**：通用化

| 轮 | 任务 | 验收 |
|---|---|---|
| 71 | Linux (x86_64) CI 构建 | GitHub Actions ubuntu-latest ✅ |
| 72 | macOS (arm64) CI 构建 | macos-14 (M1) ✅ |
| 73 | Windows (x86_64) CI 构建 | windows-latest ✅ |
| 74 | 跨平台路径处理（`dirs` crate） | 三端存储路径正确 |
| 75 | 跨平台信号处理（SIGUSR1 → Windows 等价） | 日志级别动态切换三端可用 |
| 76 | Windows 服务包装（`sc.exe` 或 WinSW） | `agent-circle daemon install` → Windows 服务 |
| 77 | macOS launchd 集成 | `agent-circle daemon install` → launchd plist |
| 78 | Linux systemd 集成 | `agent-circle daemon install` → systemd unit |
| 79 | 跨平台 E2E 测试 | 三端均通过 `tests/e2e/` |
| 80 | S07 回顾 | Sprint 回顾文档 |

---

## S08 · 协议版本化 + crates（轮 81–90）

**锚点**：系列化

| 轮 | 任务 | 验收 |
|---|---|---|
| 81 | 协议版本协商（`/agent-circle/0.1.0` → `0.2.0`） | 新老版本互通 |
| 82 | 协议语义化版本策略 | SemVer 规则文档 |
| 83 | workspace 拆分：`agent-circle-core` | 核心类型 + 身份 + 协议 |
| 84 | workspace 拆分：`agent-circle-net` | 网络层（swarm, transports） |
| 85 | workspace 拆分：`agent-circle-cli` | CLI 入口 |
| 86 | workspace 拆分：`agent-circle-timeline` | 时间线 |
| 87 | 内部 API 稳定性契约 | `core/src/lib.rs` 公开 API 文档 |
| 88 | Cargo workspace 构建提速（sccache） | CI 构建 < 5min |
| 89 | crates.io 发布 | `cargo install agent-circle` 可用 |
| 90 | S08 回顾 | Sprint 回顾文档 |

---

## S09 · Plugin 体系（轮 91–100）

**锚点**：组合化

| 轮 | 任务 | 验收 |
|---|---|---|
| 91 | Plugin trait 定义 | `trait AgentPlugin` — 生命周期钩子 |
| 92 | Plugin 发现（目录扫描） | `~/.agent-circle/plugins/*.so` 自动加载 |
| 93 | Plugin 生命周期（load/init/start/stop/unload） | 热加载/热卸载 |
| 94 | Plugin 注册自定义协议 | Plugin 可注册新的 Swarm Behaviour |
| 95 | Plugin 注册 CLI 子命令 | Plugin 可添加 `agent-circle myplugin ...` |
| 96 | Plugin 沙箱（可选 wasmtime） | Plugin 运行在 WASM 沙箱内 |
| 97 | Plugin SDK (`agent-circle-plugin` crate) | `cargo new --lib my-plugin` + 一键开发 |
| 98 | 内置 plugin: `hello-world` | 示例：收到 "hello" → 回复 "world" |
| 99 | Plugin 安全模型文档 | Plugin 权限声明 + 用户授权流程 |
| 100 | S09 回顾 | Sprint 回顾文档 |

---

## S10 · Service Discovery（轮 101–110）

**锚点**：组合化

| 轮 | 任务 | 验收 |
|---|---|---|
| 101 | Service 注册（Agent Card 扩展） | Agent Card 包含 `services: [{id, name, endpoint}]` |
| 102 | Service 广播（GossipSub 服务频道） | 上线 → 广播服务列表 |
| 103 | Service 查询（`agent-circle service search`） | 按名称/标签搜索服务 |
| 104 | Service 直连调用 | `agent-circle service call <PEER> <SERVICE>` |
| 105 | 服务发现展示层（TUI/CLI） | `agent-circle service list` 彩色表格 |
| 106 | 服务能力协商 | 调用前协商协议版本 + 参数格式 |
| 107 | "公众号" 模式：服务订阅 | `agent-circle service subscribe <SERVICE>` |
| 108 | 服务离线缓存 | 服务下线后本地缓存仍可用 |
| 109 | 服务市场概念验证 | 公共 DHT 上发布/发现服务 |
| 110 | S10 回顾 | Sprint 回顾文档 |

---

## S11 · 诊断 + 日志（轮 111–120）

**锚点**：维修性（全链路诊断）

| 轮 | 任务 | 验收 |
|---|---|---|
| 111 | `agent-circle doctor` 诊断命令 | 一键检查 identity/network/storage/contacts |
| 112 | 网络拓扑诊断（`doctor network`） | 显示 Peer 连接图 + RTT + 丢包率 |
| 113 | 存储完整性检查（`doctor storage`） | 校验 timeline.json / contacts.json |
| 114 | 协议跟踪日志（`--trace protocol`） | 每条消息收发完整 trace |
| 115 | 错误码体系（统一错误码 + 文档） | 每个 AcError 变体有唯一码 + 解释 |
| 116 | 性能指标暴露（Prometheus 格式） | `agent-circle metrics` 输出 OpenMetrics |
| 117 | 健康检查端点（本地 HTTP） | `curl http://127.0.0.1:9099/health` |
| 118 | Crash dump（panic → 结构化 dump 文件） | `~/.agent-circle/crash/2026-06-13T12:00:00.dump` |
| 119 | 远程诊断模式（`doctor --peer <ID>`） | 请求远程节点运行诊断 |
| 120 | S11 回顾 | Sprint 回顾文档 |

---

## S12 · 文档 + 打包（轮 121–130）

**锚点**：保障性（一键部署 + CI/CD）

| 轮 | 任务 | 验收 |
|---|---|---|
| 121 | 用户手册（`docs/user-guide.md`） | 从安装到发朋友圈的全流程 |
| 122 | API 文档（`docs/api/`） | 所有公开 API 有文档测试 |
| 123 | 协议规范（`docs/protocol-spec.md`） | 握手/聊天/群聊/时间线的 wire format |
| 124 | 贡献指南（`CONTRIBUTING.md`） | 开发环境搭建 → PR 流程 |
| 125 | `cargo install agent-circle` 一键装 | crates.io 发布 |
| 126 | `.deb` 打包（`cargo-deb`） | `dpkg -i agent-circle_0.3.0_amd64.deb` |
| 127 | `.rpm` 打包（`cargo-rpm`） | `rpm -i agent-circle-0.3.0.x86_64.rpm` |
| 128 | Homebrew formula | `brew install tyeclipse/tap/agent-circle` |
| 129 | Docker 镜像 | `docker run agent-circle daemon` |
| 130 | S12 回顾 | Sprint 回顾文档 |

---

## S13 · 公众号（Agent 服务发布）（轮 131–140）

**锚点**：功能补全

| 轮 | 任务 | 验收 |
|---|---|---|
| 131 | "公众号" 数据模型 | 服务描述 + 发布历史 + 订阅者列表 |
| 132 | 服务发布（`agent-circle service publish`） | 发布新版本 → 订阅者收到通知 |
| 133 | 服务订阅（`agent-circle service subscribe`） | 订阅 → 新版本推送 |
| 134 | 服务发现（`agent-circle service discover`） | DHT 上发现公共服务 |
| 135 | 推送消息（服务 → 订阅者广播） | 公众号推送 → 所有订阅者收到 |
| 136 | 消息格式（Markdown 支持） | 推送内容支持格式化 |
| 137 | 服务评级/评论 | 订阅者可对服务打分 |
| 138 | 服务市场 TUI | `agent-circle service browse` 交互式浏览 |
| 139 | 公众号权限模型 | 公开 / 需审批 / 白名单 |
| 140 | S13 回顾 | Sprint 回顾文档 |

---

## S14 · 小程序（工具共享）（轮 141–150）

**锚点**：功能补全

| 轮 | 任务 | 验收 |
|---|---|---|
| 141 | "小程序" 模型设计 | 工具描述 + 输入/输出 schema + wasm payload |
| 142 | WASM 运行时集成（wasmtime） | 加载 + 执行 WASM 模块 |
| 143 | 工具注册协议 | `agent-circle tool register` |
| 144 | 工具发现（`agent-circle tool search`） | 按名称/标签搜索工具 |
| 145 | 工具调用（`agent-circle tool invoke`） | 远程调用工具 + 结果返回 |
| 146 | 工具调用计费/速率限制 | 每 Peer 每工具调用频率限制 |
| 147 | 工具安全沙箱 | WASM 无权访问文件系统/网络（除非声明） |
| 148 | 工具市场 TUI | `agent-circle tool browse` 交互式浏览 |
| 149 | 工具版本管理 | 工具多版本共存 + 回退 |
| 150 | S14 回顾 | Sprint 回顾文档 |

---

## S15 · TUI + UX 打磨（轮 151–160）

**锚点**：功能补全

| 轮 | 任务 | 验收 |
|---|---|---|
| 151 | TUI 框架集成（ratatui） | 全屏终端界面 |
| 152 | 联系人列表 TUI | 上下键选人 → Enter 进入聊天 |
| 153 | 聊天窗口 TUI | 消息气泡 + 输入栏 + 状态栏 |
| 154 | 朋友圈 TUI | 时间线滚动 + 点赞 |
| 155 | 群聊列表 TUI | 群列表 + 未读计数 |
| 156 | 通知系统（桌面通知） | 新消息 → `notify-rust` 弹窗 |
| 157 | 快捷键体系 | `Ctrl+N` 新聊天, `Ctrl+T` 时间线, `Ctrl+Q` 退出 |
| 158 | 主题系统（亮色/暗色 + 自定义） | `agent-circle theme set dark` |
| 159 | 无障碍（屏幕阅读器兼容） | `--accessible` 模式 |
| 160 | S15 回顾 | Sprint 回顾文档 |

---

## S16 · 压力测试 + 性能（轮 161–170）

**锚点**：可靠性

| 轮 | 任务 | 验收 |
|---|---|---|
| 161 | 并发连接压测 | 1000 节点模拟 → swarm 稳定 |
| 162 | 消息吞吐量基准 | 单节点 1000 msg/s |
| 163 | 大群聊压测（100 人群组） | GossipSub mesh 稳定 |
| 164 | 时间线大容量压测（10 万条） | 10 万条 Merkle-DAG → verify < 1s |
| 165 | 内存使用 profile | 长期运行 < 100MB |
| 166 | CPU 使用 profile | 空闲 < 1% CPU |
| 167 | 启动时间优化 | 冷启动 < 2s |
| 168 | 网络带宽效率优化 | 协议开销 < 5% |
| 169 | perf 优化 sprint（flamegraph 驱动） | 识别并消除 top 3 热点 |
| 170 | S16 回顾 | Sprint 回顾文档 |

---

## S17 · 边界条件 + 长稳（轮 171–180）

**锚点**：可靠性 · 环境适应性

| 轮 | 任务 | 验收 |
|---|---|---|
| 171 | 极端 DHT 干扰（大量节点频繁上下线） | 路由表不崩溃 |
| 172 | 时钟偏移处理 | 两个节点时钟差 5min → 消息不乱序 |
| 173 | 磁盘满处理 | 磁盘满 → 优雅降级，不丢数据 |
| 174 | 超大消息处理（> 协议 MTU） | 自动分片 + 重组 |
| 175 | 7×24 长稳测试 | 连续运行 7 天 → 无内存泄漏、无崩溃 |
| 176 | 网络分区恢复 | 分区 10min → 恢复后消息自动同步 |
| 177 | PeerID 碰撞处理 | 极小概率碰撞 → 检测 + 告警 |
| 178 | IPv6 支持 | IPv6-only 环境正常运行 |
| 179 | 低速网络（2G/Edge 模拟） | > 100ms RTT + 高丢包 → 消息仍可达 |
| 180 | S17 回顾 | Sprint 回顾文档 |

---

## S18 · 集成测试 + e2e（轮 181–190）

**锚点**：测试性

| 轮 | 任务 | 验收 |
|---|---|---|
| 181 | E2E 测试框架 | 自动启动 N 个节点 → 运行场景 → 断言 |
| 182 | E2E: 身份创建 + 联系人添加 | 两节点互相发现 + 添加联系人 |
| 183 | E2E: 1对1 聊天 | 发送/接收/ACK 完整流程 |
| 184 | E2E: 群聊 | 3 节点群组 → 消息广播 |
| 185 | E2E: 朋友圈 | 创建 → 追加 → 验证 → 同步 |
| 186 | E2E: 离线消息 | A 离线 → B 发消息 → A 上线后收到 |
| 187 | E2E: NAT 穿透 | 两个 docker 容器（不同子网）→ hole-punching 成功 |
| 188 | E2E: Crash recovery | kill -9 → 重启 → 数据完整 |
| 189 | E2E: 跨版本兼容 | v0.2.0 节点 ↔ v0.3.0 节点互通 |
| 190 | S18 回顾 | Sprint 回顾文档 |

---

## S19 · 最终审计 + v1.0 发布（轮 191–200）

**锚点**：九维验收

| 轮 | 任务 | 验收 |
|---|---|---|
| 191 | 三化验收：通用化 | 三端 CI 全绿；DID 互操作 |
| 192 | 三化验收：系列化 | 协议版本协商；crates 发布；SemVer |
| 193 | 三化验收：组合化 | Plugin 热加载；Service Discovery 运行 |
| 194 | 六性验收：可靠性 | 99.9% 投递率验证报告 |
| 195 | 六性验收：维修性 + 保障性 | 诊断命令全通过；一键部署三端可用 |
| 196 | 六性验收：测试性 + 安全性 | >80% 覆盖率；fuzz clean；audit clean |
| 197 | 六性验收：环境适应性 | NAT/离线/三端/低带宽 全通过 |
| 198 | v1.0.0 发布清单 | CHANGELOG + 签名二进制 + crates.io + brew/.deb/.rpm |
| 199 | 社区建设 | CONTRIBUTING.md + CODE_OF_CONDUCT.md + 项目看板 |
| 200 | 200 轮回顾 + v1.1 方向 | 完整回顾文档 + 社区投票下一阶段 |

---

## 验收矩阵

| 维度 | 标准 | 验证方法 | 目标 Sprint |
|---|---|---|---|
| **通用化** | 三端 CI 全绿 + DID 互操作 | `cargo test --all-targets` 三端 | S07, S19 |
| **系列化** | 协议版本协商 + crates 独立发布 | 新老版本互通测试 | S08, S19 |
| **组合化** | Plugin 热加载 + Service Discovery | `agent-circle service search` | S09–S10, S19 |
| **可靠性** | 99.9% 投递 | 10000 条消息统计 | S02, S16–S17, S19 |
| **维修性** | 全链路诊断 | `agent-circle doctor` 全通过 | S11, S19 |
| **保障性** | 一键部署三端 | `cargo install` + `.deb/.rpm/brew` | S12, S19 |
| **测试性** | >80% 覆盖率 + fuzz | `cargo tarpaulin` + `cargo fuzz` | S03–S04, S18, S19 |
| **安全性** | 零严重漏洞 + E2E 审计 | `cargo audit` + 审计报告 | S05–S06, S19 |
| **环境适应性** | NAT + 离线 + 三 OS + 低带宽 | E2E + 混沌测试 | S01, S07, S17, S19 |

---

*"将孤立的任务执行者转变为协作的社交网络。"*
