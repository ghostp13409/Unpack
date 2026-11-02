use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub model: String,
    pub effort: String,
    pub user_name: String,
    pub manager_name: String,
    pub theme: String,
    /// "claude_code" | "api_key"; empty until the first readiness probe decides.
    #[serde(default)]
    pub provider: String,
    /// Optional explicit path to the Claude Code executable.
    #[serde(default)]
    pub claude_code_path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            model: "claude-haiku-5-5".to_string(),
            effort: "medium".to_string(),
            user_name: String::new(),
            manager_name: String::new(),
            theme: "system".to_string(),
            provider: String::new(),
            claude_code_path: String::new(),
        }
    }
}

const KEYS: [&str; 7] = ["model", "effort", "user_name", "manager_name", "theme", "provider", "claude_code_path"];

/// Last known readiness of the selected provider, cached so the UI has an
/// answer before the (slow) probe finishes.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProviderStatus {
    pub provider: String,
    pub ready: bool,
    pub detail: String,
    pub account: String,
    pub checked_at: String,
}

const STATUS_KEYS: [&str; 5] = [
    "provider_status.provider",
    "provider_status.ready",
    "provider_status.detail",
    "provider_status.account",
    "provider_status.checked_at",
];

/// Writes just the provider choice (used by the probe so a stale snapshot never overwrites other fields).
pub fn set_provider(conn: &Connection, provider: &str) -> rusqlite::Result<()> {
    upsert(conn, "provider", provider)
}

pub fn load_status(conn: &Connection) -> ProviderStatus {
    ProviderStatus {
        provider: get(conn, STATUS_KEYS[0]).unwrap_or_default(),
        ready: get(conn, STATUS_KEYS[1]).as_deref() == Some("true"),
        detail: get(conn, STATUS_KEYS[2]).unwrap_or_default(),
        account: get(conn, STATUS_KEYS[3]).unwrap_or_default(),
        checked_at: get(conn, STATUS_KEYS[4]).unwrap_or_default(),
    }
}

pub fn save_status(conn: &Connection, s: &ProviderStatus) -> rusqlite::Result<()> {
    let ready = if s.ready { "true" } else { "false" };
    let values = [s.provider.as_str(), ready, s.detail.as_str(), s.account.as_str(), s.checked_at.as_str()];
    for (key, value) in STATUS_KEYS.iter().zip(values) {
        upsert(conn, key, value)?;
    }
    Ok(())
}

fn upsert(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

fn get(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get::<_, String>(0))
        .ok()
}

pub fn load(conn: &Connection) -> Settings {
    let d = Settings::default();
    Settings {
        model: get(conn, "model").filter(|v| !v.trim().is_empty()).unwrap_or(d.model),
        effort: get(conn, "effort").filter(|v| !v.trim().is_empty()).unwrap_or(d.effort),
        user_name: get(conn, "user_name").unwrap_or(d.user_name),
        manager_name: get(conn, "manager_name").unwrap_or(d.manager_name),
        theme: get(conn, "theme").filter(|v| !v.trim().is_empty()).unwrap_or(d.theme),
        provider: get(conn, "provider").unwrap_or(d.provider),
        claude_code_path: get(conn, "claude_code_path").unwrap_or(d.claude_code_path),
    }
}

pub fn save(conn: &Connection, s: &Settings) -> rusqlite::Result<()> {
    let values = [&s.model, &s.effort, &s.user_name, &s.manager_name, &s.theme, &s.provider, &s.claude_code_path];
    for (key, value) in KEYS.iter().zip(values) {
        upsert(conn, key, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;

    #[test]
    fn defaults_when_nothing_saved() {
        let conn = open_memory().unwrap();
        let s = load(&conn);
        assert_eq!(s.model, "claude-haiku-5-5");
        assert_eq!(s.effort, "medium");
        assert_eq!(s.user_name, "");
        assert_eq!(s.manager_name, "");
        assert_eq!(s.theme, "system");
    }

    #[test]
    fn save_then_load_roundtrips() {
        let conn = open_memory().unwrap();
        let s = Settings {
            model: "claude-sonnet-5-5".into(),
            effort: "high".into(),
            user_name: "P".into(),
            manager_name: "M".into(),
            theme: "dark".into(),
            provider: String::new(),
            claude_code_path: String::new(),
        };
        save(&conn, &s).unwrap();
        assert_eq!(load(&conn), s);
        let v: String = conn
            .query_row("SELECT value FROM settings WHERE key='schema_version'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "1", "saving settings leaves the schema version alone");
    }

    #[test]
    fn provider_fields_default_to_unset() {
        let conn = open_memory().unwrap();
        let s = load(&conn);
        assert_eq!(s.provider, "");
        assert_eq!(s.claude_code_path, "");
    }

    #[test]
    fn provider_fields_roundtrip() {
        let conn = open_memory().unwrap();
        let s = Settings { provider: "claude_code".into(), claude_code_path: r"C:\x\claude.exe".into(), ..Settings::default() };
        save(&conn, &s).unwrap();
        assert_eq!(load(&conn), s);
    }

    #[test]
    fn status_roundtrip_and_default() {
        let conn = open_memory().unwrap();
        let d = load_status(&conn);
        assert!(!d.ready);
        assert_eq!(d, ProviderStatus::default());
        let st = ProviderStatus {
            provider: "claude_code".into(),
            ready: true,
            detail: "ok".into(),
            account: "Signed in as sam@example.com (Example Org, team plan)".into(),
            checked_at: "2026-11-01T20:00:00Z".into(),
        };
        save_status(&conn, &st).unwrap();
        assert_eq!(load_status(&conn), st);
    }
}
