//! Offline message queue backed by SQLite.
//!
//! Messages that fail to deliver are stashed here, and retried
//! when the recipient comes back online.  Expired messages
//! (past their TTL) are silently dropped on flush or cleaned
//! periodically.

use rusqlite::{params, Connection};
use std::path::Path;

#[derive(Debug, Clone)]
#[allow(dead_code)] // fields read by SQLite query_map
pub struct QueueEntry {
    pub id: i64,
    pub peer: String,
    pub content: String,
    pub created_at: String,
    pub retries: u32,
    pub last_error: Option<String>,
    /// Unix timestamp (seconds) — null means no expiry.
    pub expires_at: Option<i64>,
}

/// A persisted pending entry loaded from SQLite after crash recovery.
#[derive(Debug, Clone)]
pub struct StoredPending {
    pub request_id: u64,
    pub peer: String,
    pub from_did: String,
    pub content: String,
    pub ts: i64,
    pub msg_id: u64,
    pub ttl: i64,
    pub seq: u64,
    pub retries: u32,
    #[allow(dead_code)]
    pub created_at: i64,
}

pub struct Queue {
    #[allow(dead_code)]
    conn: Connection,
}

#[allow(dead_code)]
impl Queue {
    /// Open (or create) the offline queue database.
    pub fn open(data_dir: &Path) -> Result<Self, rusqlite::Error> {
        std::fs::create_dir_all(data_dir).ok();
        let db_path = data_dir.join("offline_queue.db");
        let conn = Connection::open(&db_path)?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS messages (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                peer        TEXT NOT NULL,
                content     TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now','localtime')),
                retries     INTEGER NOT NULL DEFAULT 0,
                last_error  TEXT,
                delivered   INTEGER NOT NULL DEFAULT 0,
                expires_at  INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_pending ON messages(peer, delivered);
            CREATE TABLE IF NOT EXISTS pending (
                request_id  INTEGER PRIMARY KEY,
                peer        TEXT NOT NULL,
                from_did    TEXT NOT NULL,
                content     TEXT NOT NULL,
                ts          INTEGER NOT NULL,
                msg_id      INTEGER NOT NULL,
                ttl         INTEGER NOT NULL,
                seq         INTEGER NOT NULL DEFAULT 0,
                retries     INTEGER NOT NULL DEFAULT 0,
                created_at  INTEGER NOT NULL
            );",
        )?;

        // Schema migration: add expires_at column on existing databases
        let _ = conn.execute_batch("ALTER TABLE messages ADD COLUMN expires_at INTEGER");

        Ok(Self { conn })
    }

    /// Push a message into the queue with an optional expiry timestamp.
    pub fn push(&self, peer: &str, content: &str) -> Result<i64, rusqlite::Error> {
        self.push_with_ttl(peer, content, None)
    }

    /// Push with a specific TTL (unix timestamp).
    pub fn push_with_ttl(
        &self,
        peer: &str,
        content: &str,
        expires_at: Option<i64>,
    ) -> Result<i64, rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO messages (peer, content, expires_at) VALUES (?1, ?2, ?3)",
            params![peer, content, expires_at],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Return all pending (undelivered, NOT expired) messages for a peer.
    pub fn pending_for(&self, peer: &str) -> Result<Vec<QueueEntry>, rusqlite::Error> {
        let now = chrono::Utc::now().timestamp();
        let mut stmt = self.conn.prepare(
            "SELECT id, peer, content, created_at, retries, last_error, expires_at
             FROM messages
             WHERE peer = ?1 AND delivered = 0
               AND (expires_at IS NULL OR expires_at > ?2)
             ORDER BY id",
        )?;
        let rows = stmt.query_map(params![peer, now], |row| {
            Ok(QueueEntry {
                id: row.get(0)?,
                peer: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
                retries: row.get(4)?,
                last_error: row.get(5)?,
                expires_at: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    /// Mark a message as delivered.
    pub fn mark_delivered(&self, id: i64) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE messages SET delivered = 1 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    /// Mark a message as failed and record the error.
    pub fn mark_failed(&self, id: i64, error: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "UPDATE messages SET retries = retries + 1, last_error = ?1 WHERE id = ?2",
            params![error, id],
        )?;
        Ok(())
    }

    /// Return queue stats: (pending, delivered, failed>3)
    pub fn stats(&self) -> Result<(i64, i64, i64), rusqlite::Error> {
        let pending: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE delivered = 0",
            [],
            |r| r.get(0),
        )?;
        let delivered: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE delivered = 1",
            [],
            |r| r.get(0),
        )?;
        let failed: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE retries > 3 AND delivered = 0",
            [],
            |r| r.get(0),
        )?;
        Ok((pending, delivered, failed))
    }

    /// Flush ALL pending (non-expired) messages.
    pub fn all_pending(&self) -> Result<Vec<QueueEntry>, rusqlite::Error> {
        let now = chrono::Utc::now().timestamp();
        let mut stmt = self.conn.prepare(
            "SELECT id, peer, content, created_at, retries, last_error, expires_at
             FROM messages
             WHERE delivered = 0
               AND (expires_at IS NULL OR expires_at > ?1)
             ORDER BY peer, id",
        )?;
        let rows = stmt.query_map(params![now], |row| {
            Ok(QueueEntry {
                id: row.get(0)?,
                peer: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
                retries: row.get(4)?,
                last_error: row.get(5)?,
                expires_at: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    /// Delete expired messages. Returns number of rows removed.
    pub fn expire_before(&self, cutoff_ts: i64) -> Result<usize, rusqlite::Error> {
        let n = self.conn.execute(
            "DELETE FROM messages WHERE expires_at IS NOT NULL AND expires_at <= ?1 AND delivered = 0",
            params![cutoff_ts],
        )?;
        Ok(n)
    }

    /// Delete ALL delivered messages (vacuum helper).
    pub fn prune_delivered(&self) -> Result<usize, rusqlite::Error> {
        let n = self
            .conn
            .execute("DELETE FROM messages WHERE delivered = 1", [])?;
        Ok(n)
    }

    // ── Pending tracker persistence ──────────────────────────────

    /// Persist an in-flight message so it survives daemon crashes.
    #[allow(clippy::too_many_arguments)]
    pub fn push_pending(
        &self,
        request_id: u64,
        peer: &str,
        from_did: &str,
        content: &str,
        ts: i64,
        msg_id: u64,
        ttl: i64,
        seq: u64,
        retries: u32,
    ) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "INSERT OR REPLACE INTO pending (request_id, peer, from_did, content, ts, msg_id, ttl, seq, retries, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                request_id as i64,
                peer,
                from_did,
                content,
                ts,
                msg_id as i64,
                ttl,
                seq as i64,
                retries,
                chrono::Utc::now().timestamp(),
            ],
        )?;
        Ok(())
    }

    /// Remove a persisted pending entry (ACK received or retries exhausted).
    pub fn remove_pending(&self, request_id: u64) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM pending WHERE request_id = ?1",
            params![request_id as i64],
        )?;
        Ok(())
    }

    /// Remove a persisted pending entry by peer + msg_id (for retrack).
    pub fn remove_pending_by_msg(&self, peer: &str, msg_id: u64) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM pending WHERE peer = ?1 AND msg_id = ?2",
            params![peer, msg_id as i64],
        )?;
        Ok(())
    }

    /// Load all persisted pending entries (for crash recovery on startup).
    pub fn load_all_pending(&self) -> Result<Vec<StoredPending>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT request_id, peer, from_did, content, ts, msg_id, ttl, seq, retries, created_at
             FROM pending ORDER BY created_at",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(StoredPending {
                request_id: row.get::<_, i64>(0)? as u64,
                peer: row.get(1)?,
                from_did: row.get(2)?,
                content: row.get(3)?,
                ts: row.get(4)?,
                msg_id: row.get::<_, i64>(5)? as u64,
                ttl: row.get(6)?,
                seq: row.get::<_, i64>(7)? as u64,
                retries: row.get::<_, i64>(8)? as u32,
                created_at: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    /// Delete all expired pending entries. Returns number removed.
    pub fn expire_pending(&self, cutoff_ts: i64) -> Result<usize, rusqlite::Error> {
        let n = self
            .conn
            .execute("DELETE FROM pending WHERE ttl < ?1", params![cutoff_ts])?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_queue() -> Queue {
        let dir = std::env::temp_dir().join(format!("ac_test_{}", rand::random::<u32>()));
        Queue::open(&dir).unwrap()
    }

    #[test]
    fn push_and_remove_pending() {
        let q = temp_queue();
        q.push_pending(1, "peer_a", "alice", "hello", 100, 42, 9999999999, 1, 0)
            .unwrap();
        let all = q.load_all_pending().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].content, "hello");

        q.remove_pending(1).unwrap();
        assert!(q.load_all_pending().unwrap().is_empty());
    }

    #[test]
    fn remove_nonexistent_ok() {
        let q = temp_queue();
        assert!(q.remove_pending(999).is_ok());
    }

    #[test]
    fn remove_by_msg_id() {
        let q = temp_queue();
        q.push_pending(1, "peer_a", "a", "msg1", 1, 100, 999, 1, 0)
            .unwrap();
        q.push_pending(2, "peer_b", "b", "msg2", 2, 200, 999, 1, 0)
            .unwrap();
        q.remove_pending_by_msg("peer_a", 100).unwrap();
        let all = q.load_all_pending().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].peer, "peer_b");
    }

    #[test]
    fn insert_or_replace() {
        let q = temp_queue();
        q.push_pending(1, "peer", "a", "old", 1, 1, 999, 1, 0)
            .unwrap();
        q.push_pending(1, "peer", "a", "new", 2, 1, 999, 1, 0)
            .unwrap();
        let all = q.load_all_pending().unwrap();
        assert_eq!(all.len(), 1);
        // INSERT OR REPLACE should replace with new content
        assert_eq!(all[0].content, "new");
        assert_eq!(all[0].ts, 2);
    }

    #[test]
    fn expire_pending_by_ttl() {
        let q = temp_queue();
        q.push_pending(1, "p", "a", "will_expire", 1, 1, 100, 1, 0)
            .unwrap();
        q.push_pending(2, "p", "b", "still_alive", 2, 2, 9999999999, 1, 0)
            .unwrap();
        let n = q.expire_pending(200).unwrap();
        assert_eq!(n, 1);
        let all = q.load_all_pending().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].content, "still_alive");
    }

    #[test]
    fn crash_recovery_load_and_clear() {
        let q = temp_queue();
        q.push_pending(10, "p1", "a", "m1", 1, 1, 9999999999, 1, 0)
            .unwrap();
        q.push_pending(20, "p2", "b", "m2", 2, 2, 9999999999, 1, 1)
            .unwrap();
        let stored = q.load_all_pending().unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].retries, 0);
        assert_eq!(stored[1].retries, 1);

        // Clear all
        for sp in &stored {
            q.remove_pending(sp.request_id).unwrap();
        }
        assert!(q.load_all_pending().unwrap().is_empty());
    }
}
