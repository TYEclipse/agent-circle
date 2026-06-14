# S02R24: 投递状态回调 — `chat send --track` 实时显示送达状态

> **目标**：`agent-circle chat send --track <peer> "msg"` → 等待并显示投递结果
> 三种状态：✅ Delivered / ❌ Failed / ⏰ Pending (timeout)

**当前问题**：`cmd_chat_send` fire-and-forget，不追踪投递结果。

**方案**：
1. upgrade `send_chat()` → 返回 `OutboundRequestId`
2. `cmd_chat_send` 加 `--track` flag → 发送后进入事件循环等待 ACK
3. 收 ACK → print "✅ Delivered in Xms"
4. 收 OutboundFailure → print "❌ Failed: {error}"
5. 超时 → print "⏰ Pending (no ACK within {timeout}s)"

### Task 1: `send_chat()` 返回 request_id
### Task 2: `cmd_chat_send` 加 `--track` + `--timeout` flags
### Task 3: 事件循环等待 ACK or Failure
### Task 4: 测试 `--track` 行为
### Task 5: 编译 + test + fmt + clippy 全绿
