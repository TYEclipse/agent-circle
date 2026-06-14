# S02R22: 消息序列号 + 顺序保证

> **目标**：发送端按序编号每条消息，接收端缓冲乱序消息并按正确顺序投递。
> ROADMAP: "乱序消息自动重排"

## 问题
发送 msg A(seq=1) 后 msg B(seq=2)。A 重试延迟 → B 先到 → 接收端看到 B 先于 A。
需要在接收端按 seq 重排。

## 设计

### ChatRequest 新增字段
```rust
pub seq: u64,  // 发送端单调递增，crash 后重置
```

### SequenceTracker (新模块)
- `last_seq: HashMap<PeerId, u64>` — 每个 sender 最后收到的 seq
- `buffer: HashMap<PeerId, BTreeMap<u64, ChatRequest>>` — 乱序消息暂存
- 收到 `seq = last+1` → 立即投递 + 冲刷缓冲区
- 收到 `seq > last+1` → 缓冲区暂存
- 收到 `seq <= last` → 丢弃（重放/重复，dedup 层会先过滤）
- ConnectionEstablished → 重置该 peer 的状态（seq 计数器重启了）

### 发送端
- `AtomicU64` 计数器，每次 `send_chat()` 递增

## Task 清单
1. `ChatRequest` + `seq`
2. 新建 `src/sequence.rs` — `SequenceTracker`
3. daemon 集成 — 接收端重排、发送端自增、连接时重置
4. 测试
5. 编译 + fmt + clippy + test + CHANGELOG + commit
