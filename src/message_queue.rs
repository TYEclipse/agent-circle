//! Offline message queue backed by SQLite.
//!
//! Messages that fail to deliver are stashed here, and retried
//! when the recipient comes back online.

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
}

pub struct Queue {
    conn: Connection,
}

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
                delivered   INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_pending ON messages(peer, delivered);",
        )?;

        Ok(Self { conn })
    }

    /// Push a message into the queue.
    pub fn push(&self, peer: &str, content: &str) -> Result<i64, rusqlite::Error> {
        self.conn.execute(
            "INSERT INTO messages (peer, content) VALUES (?1, ?2)",
            params![peer, content],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Return all pending (undelivered) messages for a peer.
    pub fn pending_for(&self, peer: &str) -> Result<Vec<QueueEntry>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, peer, content, created_at, retries, last_error
             FROM messages
             WHERE peer = ?1 AND delivered = 0
             ORDER BY id",
        )?;
        let rows = stmt.query_map(params![peer], |row| {
            Ok(QueueEntry {
                id: row.get(0)?,
                peer: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
                retries: row.get(4)?,
                last_error: row.get(5)?,
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
        let pending: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM messages WHERE delivered = 0", [], |r| r.get(0))?;
        let delivered: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM messages WHERE delivered = 1", [], |r| r.get(0))?;
        let failed: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM messages WHERE retries > 3 AND delivered = 0", [], |r| {
                r.get(0)
            })?;
        Ok((pending, delivered, failed))
    }

    /// Flush ALL pending messages (try-send regardless of peer).
    pub fn all_pending(&self) -> Result<Vec<QueueEntry>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, peer, content, created_at, retries, last_error
             FROM messages
             WHERE delivered = 0
             ORDER BY peer, id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(QueueEntry {
                id: row.get(0)?,
                peer: row.get(1)?,
                content: row.get(2)?,
                created_at: row.get(3)?,
                retries: row.get(4)?,
                last_error: row.get(5)?,
            })
        })?;
        rows.collect()
    }
}
