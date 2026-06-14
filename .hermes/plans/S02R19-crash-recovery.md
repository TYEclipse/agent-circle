# S02R19: 崩溃恢复 — 持久化 PendingTracker 到 SQLite

> **目标**：daemon 被 kill -9 后重启，自动恢复所有未 ACK 消息并重新发送。

**问题**：PendingTracker 纯内存。kill -9 → 所有 in-flight 消息丢失（还未入离线队列的）。

**方案**：将 PendingTracker 的 track/ack/retrack 操作同步到 SQLite。重启时加载并重发。

---

### 数据模型

在 `messages` 表旁新增 `pending` 表：

```sql
CREATE TABLE IF NOT EXISTS pending (
    request_id  INTEGER PRIMARY KEY,  -- OutboundRequestId 的 u64
    peer        TEXT NOT NULL,
    from_did    TEXT NOT NULL,
    content     TEXT NOT NULL,
    ts          INTEGER NOT NULL,
    msg_id      INTEGER NOT NULL,
    ttl         INTEGER NOT NULL,
    retries     INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL      -- unix timestamp seconds
);
```

### Task 1: 扩展 SQLite schema — 新增 `pending` 表

**文件**: `src/message_queue.rs`

在 `Queue::open()` 中创建 `pending` 表 + `push_pending` / `remove_pending` / `load_all_pending` 方法。

### Task 2: PendingTracker 同步 SQLite

**文件**: `src/reliability.rs`

- `track()` 调用后同步写入 SQLite
- `ack()` 移除后同步删除 SQLite 行
- `retrack()` 更新 SQLite（先删旧 request_id，后插新 request_id）
- `fail()` + 重试耗尽 → 删除 SQLite 行（已移交离线队列）

### Task 3: Daemon 启动恢复

**文件**: `src/network.rs` (`run_daemon`)

- 启动后 `load_all_pending()` → 遍历每条 → 重新 `send_request` + `track()`
- 恢复前的 pending 可能有不同的 request_id（libp2p 每次 send_request 生成新 ID），启动后用新的 request_id 重新发送

### Task 4: 单元测试

**文件**: `src/message_queue.rs` (tests) + `src/reliability.rs` (tests)

- 测试 SQLite pending 表的基本 CRUD
- 测试 tracker 操作后数据库一致性
- 测试重复加载不会造成重复发送

### Task 5: 编译 + test + fmt + clippy 全绿
