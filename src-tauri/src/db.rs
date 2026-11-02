use rusqlite::Connection;
use std::path::Path;

const SCHEMA_VERSION: i64 = 1;

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = Connection::open(path)?;
    // The window, the MCP server and the command-line modes can each hold a
    // connection; wait for another writer instead of failing at once.
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_memory() -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )?;
    let version: i64 = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    if version < 1 {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'todo',
                priority TEXT NOT NULL DEFAULT 'medium',
                category TEXT,
                due_date TEXT,
                parent_id TEXT REFERENCES tasks(id),
                pinned INTEGER NOT NULL DEFAULT 0,
                snoozed_until TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                completed_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_parent ON tasks(parent_id);

            CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL REFERENCES tasks(id),
                body TEXT NOT NULL,
                source_capture_id TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_notes_task ON notes(task_id);

            CREATE TABLE IF NOT EXISTS captures (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                summary TEXT,
                model TEXT NOT NULL,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'proposed',
                proposal_json TEXT
            );

            CREATE TABLE IF NOT EXISTS capture_inputs (
                id TEXT PRIMARY KEY,
                capture_id TEXT NOT NULL REFERENCES captures(id),
                kind TEXT NOT NULL,
                filename TEXT,
                extracted_text TEXT NOT NULL,
                char_count INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_inputs_capture ON capture_inputs(capture_id);

            CREATE TABLE IF NOT EXISTS changes (
                id TEXT PRIMARY KEY,
                capture_id TEXT NOT NULL REFERENCES captures(id),
                task_id TEXT,
                kind TEXT NOT NULL,
                before_json TEXT NOT NULL DEFAULT '{}',
                after_json TEXT NOT NULL DEFAULT '{}',
                reason TEXT NOT NULL DEFAULT '',
                evidence TEXT,
                applied INTEGER NOT NULL DEFAULT 0,
                undone INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_changes_capture ON changes(capture_id);
            CREATE INDEX IF NOT EXISTS idx_changes_task ON changes(task_id);
            ",
        )?;
    }

    conn.execute(
        "INSERT INTO settings(key, value) VALUES ('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [SCHEMA_VERSION.to_string()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_is_idempotent_and_records_version() {
        let conn = open_memory().unwrap();
        migrate(&conn).unwrap();
        let v: String = conn
            .query_row("SELECT value FROM settings WHERE key='schema_version'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION.to_string());
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('tasks','notes','captures','capture_inputs','changes','settings')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 6);
    }
}
