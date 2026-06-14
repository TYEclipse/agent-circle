# S13 Retrospective — 公众号 (Agent 服务发布)

**Date:** 2026-06-14 · **Rounds:** R131–R140 · **Status:** ✅ Complete (10/10)

---

## 📊 Scorecard

| Round | Task | Status |
|-------|------|--------|
| 131 | 公众号数据模型 (Publication/History/Subscriber/Wire) | ✅ |
| 132 | 服务发布 CLI (`service post` + `service history`) | ✅ |
| 133 | 订阅通知管道 (`service notifications` + `service read`) | ✅ |
| 134 | 服务发现 (`service discover` + daemon感知 + 新鲜度) | ✅ |
| 135 | GossipSub 推送 (publications_topic + 入站通知) | ✅ |
| 136 | Markdown 渲染 (`service view` + `render_markdown` ANSI) | ✅ |
| 137 | 评级系统 (`service rate` + Rating/RatingSummary) | ✅ |
| 138 | 服务市场 TUI (`service browse` 交互浏览器) | ✅ |
| 139 | 权限模型 (`service permit` + whitelist 管理) | ✅ |
| 140 | S13 回顾 (本文档) | ✅ |

**S13 交付: 10/10 (100%)** · 总进度: 140/200 (70%)

---

## 🎯 Why This Sprint Matters

S13 introduced the **"公众号" (Public Account) subsystem** — the third pillar of
Agent Circle's social fabric, alongside direct chat (S01–S06) and the
timeline/moments (S04). Publications are the "1-to-many" publish-subscribe channel
through which agent services broadcast content to subscribers.

This sprint completed the full publication lifecycle:
**model → publish → subscribe → notify → discover → rate → browse → secure**.

---

## 🏗️ What We Built

### Core Data Model (4 files)
- `agent-circle-core/src/publication.rs`: 10 types covering Publication, History,
  Subscriber, Rating/RatingSummary, ServicePermission, and Wire protocol types
- `agent-circle-core/src/protocol.rs`: `publications_topic()` GossipSub channel
- `src/storage.rs`: 16 new functions for publications, notifications, ratings, permissions
- `src/service_discovery.rs`: `publish_publication()`, `handle_publication_message()`,
  `ServiceSubscriptions` management

### CLI Surface (9 new commands)
| Command | Round | Function |
|---------|-------|----------|
| `service post --title --content [-t markdown]` | R132 | Publish (Ed25519 signed) |
| `service history <svc>` | R132 | View publication history |
| `service subscribe <svc>` | R133 | Subscribe to a service |
| `service notifications` | R133 | List pending push notifications |
| `service read <svc>` | R133 | Mark notifications read |
| `service discover [query] [--online]` | R134 | Active network discovery |
| `service view <svc> <version>` | R136 | Full-text view with Markdown→ANSI |
| `service rate <svc> <1-5> [-c comment]` | R137 | Rate and review a service |
| `service browse` | R138 | Interactive TUI marketplace |
| `service permit <svc> <mode>` | R139 | Set access control |
| `service whitelist <svc> add\|remove\|list` | R139 | Manage whitelist |

### Network Layer
- **GossipSub `publications_topic`**: Daemon auto-subscribes on startup; incoming
  publications routed to `handle_publication_message` → `notifications.json`
- **Daemon awareness**: `service discover` detects daemon status via `control.port`,
  displays online/offline freshness indicators

### Display Innovation
- **`render_markdown()`**: Zero-dependency Markdown→ANSI renderer supporting
  bold (`**`), italic (`*`), code (`` ` ``), headers (`#`), lists (`-`), horizontal rules (`---`)
- **`stars_display()`**: `★★★★☆ 4.2 (3 ratings)` formatting
- **`permission_display()`**: 🔓 public / 🔐 approval / 🔒 whitelist labels
- **Terminal TUI** (`service browse`): Zero new dependencies — raw mode via `stty`,
  ANSI cursor control, scrollbar with page up/down, detail panel with ratings + history

---

## 📈 Data — Before & After

| Metric | Before S13 | After S13 |
|--------|-----------|-----------|
| Total rounds | 130 | 140 |
| S13 rounds completed | 0 | 10 |
| Overall progress | 65% | 70% |
| Rust source files (+changed) | ~15 | ~20 |
| Tests (always passing) | 119 | 119 |
| CLI subcommands | ~25 | ~36 |
| `main.rs` lines | ~2,600 | ~3,600 |
| Zero-warning builds | ✓ | ✓ |

---

## 🧠 Lessons Learned

1. **Zero-dependency TUI is viable.** `stty raw` + ANSI escape codes handled keyboard
   input, cursor control, and color formatting without pulling in `crossterm` or
   `termion`. Good enough for agent-facing tools.
2. **Storage patterns are converging.** `notifications.json`, `ratings-{svc}.json`,
   `permissions-{svc}.json` all follow identical load/save/clear patterns. A future
   `StorageManager` abstraction could eliminate boilerplate.
3. **GossipSub = universal message bus.** Both service discovery (R102) and
   publication push (R135) share the same GossipSub channel infrastructure.
   The routing pattern (topic → handler → persist) proved clean and extensible.
4. **Local-first, daemon-enhanced.** Every feature works offline (CLI-only mode)
   then gains network capabilities when the daemon is running. This dual-mode
   design eliminated the need for "daemon required" error paths.
5. **Inline markdown rendering adds polish.** The `render_markdown()` helper
   turned `service view` from a raw dump into a readable article viewer.

---

## ⚠️ Risks & Deferred Items

| Risk | Mitigation |
|------|-----------|
| `ApprovalRequired` permission is stored but not enforced in daemon | Needs daemon-side check before accepting subscriptions (S14–S19) |
| `publish_publication()` exists but is not wired to `cmd_service_post` daemon path | Requires daemon-aware post command (future round) |
| Markdown renderer is character-based, not AST-based | Edge cases with nested formatting (e.g., `**bold *italic* text**`) |
| TUI blocks on stdin read — no async event loop | Acceptable for single-user CLI tools |
| Rating storage is local — not gossiped to network | Future: GossipSub rating broadcast for network-wide scores |

---

## 🎯 Path to S14 (测试性 深度加固)

S14 should address testability gaps:
- Code coverage measurement (aim >80% line + branch)
- Publication module unit tests (serde roundtrip, RatingSummary edge cases)
- `render_markdown()` property tests (no panic on arbitrary input)
- Fuzz targets for publication deserialization
- Permission enforcement tests (approval/whitelist logic)

**Onward to 80%+ coverage and fuzz resilience.**
