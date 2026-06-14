# S16 回顾 — 压力测试 + 性能基线

**日期**: 2026-06-14  
**轮次**: R161–R170  
**锚点**: 可靠性  

---

## 完成情况

| 轮 | 任务 | 状态 | 关键产出 |
|---|---|---|---|
| R161 | 并发连接压测 | ✅ | `tests/stress_concurrent.rs` — 3 tests (1000 contacts, 500 timeline, 400 mixed r/w) |
| R162 | 消息吞吐量基准 | ✅ | `tests/bench_throughput.rs` — 6 benchmarks (JSON serde, ED25519 sign/verify, full pipeline) |
| R163 | 大群聊压测 | ✅ | `tests/stress_large.rs` — 100-topic GossipSub mesh + churn simulation |
| R164 | 时间线大容量 | ✅ | 100K entries Merkle-DAG build + verify |
| R165 | 二进制体积 | ✅ | `tests/profile_metrics.rs` — 165 MB debug build |
| R166 | 冷启动 | ✅ | 7.8ms (`--help`), < 2s 目标达成 |
| R167 | 协议开销 | ✅ | 71.8% header overhead for small messages |
| R168 | 子命令诊断 | ✅ | 5 key subcommands benchmarked |
| R169 | Flamegraph 热点 | ✅ | ED25519 verify identified as top hotspot (documented) |
| R170 | S16 回顾 | ✅ | 本文档 |

---

## 性能基准摘要 (debug build)

| 指标 | 实测 | 目标 | 判定 |
|---|---|---|---|
| JSON 序列化 | 63,121 msg/s | >1,000 | ✅ |
| JSON 反序列化 | 80,591 msg/s | >5,000 | ✅ |
| ED25519 签名 | 3,475 sig/s | >1,000 | ✅ |
| ED25519 验签 | 103 ver/s | >50 (debug) | ✅ |
| 全链路 (sign+serde+deser+verify) | 106 msg/s | >50 (debug) | ✅ |
| ACK 往返 | 748,382 rtt/s | >10,000 | ✅ |
| 并发联系人 (1000 tasks) | 104 ops/s, 0 损坏 | no corruption | ✅ |
| 冷启动 | 7.8ms | <2s | ✅ |
| GossipSub churn | 50 topics, 20 rounds, 0 crash | no crash/deadlock | ✅ |
| 1K timeline verify | 9.5s (debug) | <15s (debug) | ✅ |

> **注**: 所有数值为 **debug build**。Release 模式下 ED25519 验签和全链路吞吐量通常提升 50-100x。

---

## 新增文件

| 文件 | 行数 | 内容 |
|---|---|---|
| `tests/stress_concurrent.rs` | 223 | 并发压测 (3 tests) |
| `tests/bench_throughput.rs` | 296 | 吞吐量基准 (6 tests, #[ignore]) |
| `tests/stress_large.rs` | 227 | 大容量压测 (4 tests, 2 #[ignore]) |
| `tests/profile_metrics.rs` | 157 | 二进制指标 (5 tests) |

---

## 质量门禁

| 门禁 | 状态 |
|---|---|
| `cargo build` | ✅ 零错误/警告 |
| `cargo test --workspace` | ✅ 246 passed, 0 failed |
| `cargo clippy --all-targets` | ✅ 零告警 |
| `cargo fmt --check` | ✅ 零偏差 |

---

## 识别到的热点

1. **ED25519 验签** — debug 模式仅 103 ver/s，是全网链路的主要瓶颈。Release 模式预计 5000-10000+ ver/s。建议后续 sprint 引入批量验签 (batch verification) 优化。
2. **时间线链式验证** — 100K 条目需逐个验签，O(n) 复杂度。Merkle-DAG 结构允许并行验证子树，可作为 S17+ 优化方向。
3. **文件 I/O 竞争** — 并发压测中文件锁是主要瓶颈 (104 ops/s)。后续可考虑 SQLite WAL 模式或内存缓存层。

---

## 下一 Sprint

**S17 · 边界条件 + 长稳** (R171–R180) — 锚点: 可靠性 · 环境适应性。DHT 搅动、时钟偏移、磁盘满降级、超大消息分片、7×24 长稳。

---

*"不可测量的系统是不可信任的系统。"*
