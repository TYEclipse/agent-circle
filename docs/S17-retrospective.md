# S17 回顾 — 边界条件 + 长稳（轮 171–180）

**日期**: 2026-06-15  
**轮次**: R171–R180  
**状态**: ✅ 闭合  
**总进度**: 180/200 (90%)

---

## 目标

验证 agent-circle 在极端条件下的正确性：
- DHT 搅动时路由表不崩溃
- 时钟偏移不影响消息顺序
- 磁盘满时优雅降级不丢数据
- 超大消息自动分片重组
- 长时间运行无内存泄漏、无崩溃
- 网络分区恢复后消息同步
- PeerID 碰撞检测
- IPv6 支持
- 低速网络容忍

## 完成情况

| 轮 | 任务 | 测试 | 状态 |
|---|---|---|---|
| R171 | 极端 DHT 搅动 | `dht_churn.rs` 5 tests | ✅ |
| R172 | 时钟偏移处理 | `clock_skew.rs` 7 tests | ✅ |
| R173 | 磁盘满处理 | `src/disk.rs` 9 tests + E0006 | ✅ |
| R174 | 超大消息分片 | `src/fragment.rs` 12 tests | ✅ |
| R175 | 7×24 长稳模拟 | ~100k 连续操作无泄漏 | ✅ |
| R176 | 网络分区恢复 | 序列重置 + 离线队列 | ✅ |
| R177 | PeerID 碰撞检测 | 1000 唯一性验证 | ✅ |
| R178 | IPv6 支持 | 4 Multiaddr 解析测试 | ✅ |
| R179 | 低速网络容忍 | 重排序 + 去重 + TTL | ✅ |
| R180 | S17 回顾 | 本文件 | ✅ |

## 新增代码

### 新模块
| 文件 | 描述 |
|---|---|
| `src/disk.rs` | 磁盘空间监测与降级（libc statvfs） |
| `src/fragment.rs` | 64KB 消息分片/重组引擎 |

### 修改
| 文件 | 变更 |
|---|---|
| `agent-circle-core/src/errors.rs` | +E0006 DiskFull |
| `agent-circle-core/src/chat.rs` | ServiceCall +fragment_info |
| `src/lib.rs` | +pub mod disk, +pub mod fragment |
| `Cargo.toml` | +libc |

### 新测试文件
| 文件 | 测试数 | 覆盖轮次 |
|---|---|---|
| `tests/dht_churn.rs` | 5 | R171 |
| `tests/clock_skew.rs` | 7 | R172 |
| `tests/s17_boundary.rs` | 19 | R175–R179 |

## 指标

| 指标 | 值 |
|---|---|
| S17 新增测试 | **43** (5+7+12+19) |
| 全量测试 | **~296** (S16: 246 → S17: 296) |
| 新增源文件 | `disk.rs`, `fragment.rs` |
| 新增 lib 依赖 | `libc` |
| 质量门禁 | ✅ build · ✅ clippy 0 · ✅ fmt |

## 关键决策

- 时钟偏移：seq-based 排序天然免疫 ts 偏移，无需额外逻辑
- 分片策略：使用 ServiceCall.fragment_info 编码片段信息，避免协议变更
- 分片大小：64KB（libp2p 友好），重组 TTL 60s
- 磁盘阈值：10MB critical / 100MB warning
- IPv6：libp2p Multiaddr 原生支持，仅需测试验证解析
- 7×24 长稳无法实际跑 7 天，采用加速模拟（100k operations）

## 经验教训

- MemoryStore 默认 max_records 偏小，长稳测试需配置 MemoryStoreConfig
- 分片重组需处理乱序到达（网络延迟不确定），HashMap 收集 + 计数验证
- 磁盘可用空间查询需 libc::statvfs（非 posix 标准 API）

---

**S17 闭合。总进度 180/200 (90%)。**
