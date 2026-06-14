//! Offline message queue backed by SQLite.
//!
//! Messages that fail to deliver are stashed here, and retried
//! when the recipient comes back online.  Expired messages
//! (past their TTL) are silently dropped on flush or cleaned
//! periodically.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            CREATE INDEX IF NOT EXISTS idx_pending ON messages(peer, delivered);",
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
}
