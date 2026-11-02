//! Command-line modes that work on the database without opening the window:
//!
//! ```text
//! unpack --probe --out <file>
//! unpack --list-tasks --out <file>
//! unpack --import <proposal.json> --out <file> [--as <label>]
//! unpack --set-status <task_id> <todo|in_progress|done> --out <file>
//! ```
//!
//! Results always go to the `--out` file as JSON (a GUI-subsystem executable
//! has no console to print to): `{"ok": true, ...}` or `{"ok": false, "error": "..."}`.
//! The exit code is 0 on success and 1 on any error.

use crate::ai::ops::{resolve, ModelResponse};
use crate::apply::apply_proposal;
use crate::commands::compute_status;
use crate::models::{Change, InputKind, Status, Task, TaskPatch};
use crate::repo::{captures, tasks};
use crate::settings::ProviderStatus;
use crate::{db, secrets, settings};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

const MODES: [&str; 4] = ["--probe", "--list-tasks", "--import", "--set-status"];

/// `%APPDATA%\<identifier>`, the same folder Tauri resolves as the app data dir on Windows.
pub fn data_dir(identifier: &str) -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join(identifier)
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

/// Runs a headless mode when the arguments ask for one. `None` means "open the window as usual".
pub fn maybe_run(identifier: &str, args: &[String]) -> Option<i32> {
    let mode = args.get(1)?;
    if !MODES.contains(&mode.as_str()) {
        return None;
    }
    let out = flag_value(args, "--out");
    let result = run_mode(identifier, mode, args);
    let (code, body) = match result {
        Ok(v) => (0, v),
        Err(e) => (1, json!({"ok": false, "error": e})),
    };
    match out {
        Some(path) => {
            if let Err(e) = std::fs::write(&path, serde_json::to_string_pretty(&body).unwrap_or_default()) {
                eprintln!("could not write {path}: {e}");
                return Some(1);
            }
        }
        None => println!("{body}"),
    }
    Some(code)
}

fn run_mode(identifier: &str, mode: &str, args: &[String]) -> Result<Value, String> {
    let dir = data_dir(identifier);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create the data folder: {e}"))?;
    let conn = db::open(&dir.join("tasks.db")).map_err(|e| format!("Database error: {e}"))?;
    match mode {
        "--probe" => Ok(json!({"ok": true, "probe": probe(&conn)?})),
        "--list-tasks" => {
            let all = tasks::list_tasks(&conn, true).map_err(|e| format!("Database error: {e}"))?;
            Ok(json!({"ok": true, "tasks": all}))
        }
        "--import" => {
            let file = args.get(2).filter(|a| !a.starts_with("--")).ok_or("Give the proposal file after --import.")?;
            let text = std::fs::read_to_string(file).map_err(|e| format!("Could not read {file}: {e}"))?;
            let label = flag_value(args, "--as").unwrap_or_else(|| "manual-import".to_string());
            let filename = Path::new(file).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| file.clone());
            let (capture_id, changes, warnings) = import_proposal(&conn, &text, &filename, &label)?;
            Ok(json!({"ok": true, "capture_id": capture_id, "changes": changes, "warnings": warnings}))
        }
        "--set-status" => {
            let id = args.get(2).ok_or("Give the task id and the status after --set-status.")?;
            let status = args.get(3).ok_or("Give the status after the task id.")?;
            let task = set_status(&conn, id, status)?;
            Ok(json!({"ok": true, "task": task}))
        }
        _ => Err(format!("Unknown mode {mode}")),
    }
}

/// Re-checks the selected provider and stores the result (and an auto-chosen provider).
pub fn probe(conn: &Connection) -> Result<ProviderStatus, String> {
    let s = settings::load(conn);
    let has_key = secrets::get_api_key().unwrap_or(None).is_some();
    let status = tauri::async_runtime::block_on(compute_status(&s, has_key));
    settings::save_status(conn, &status).map_err(|e| format!("Database error: {e}"))?;
    if s.provider != status.provider {
        settings::set_provider(conn, &status.provider).map_err(|e| format!("Database error: {e}"))?;
    }
    Ok(status)
}

/// Stores a proposal in the model's response shape as a capture and applies every operation.
pub fn import_proposal(conn: &Connection, text: &str, filename: &str, label: &str) -> Result<(String, Vec<Change>, Vec<String>), String> {
    let resp: ModelResponse = serde_json::from_str(text).map_err(|e| format!("The proposal file could not be read: {e}"))?;
    import_response(conn, resp, text, filename, label)
}

/// Records `resp` as a capture (with `text` as its input) and applies every operation.
/// Returns the capture id, the changes made, and warnings about operations that were skipped.
pub fn import_response(
    conn: &Connection,
    resp: ModelResponse,
    text: &str,
    filename: &str,
    label: &str,
) -> Result<(String, Vec<Change>, Vec<String>), String> {
    let db_err = |e: rusqlite::Error| format!("Database error: {e}");
    let open: HashMap<String, Option<String>> = tasks::list_tasks(conn, false)
        .map_err(db_err)?
        .into_iter()
        .map(|t| (t.id, t.parent_id))
        .collect();
    let proposal = resolve(resp, &open);
    let capture = captures::insert_capture(conn, label).map_err(db_err)?;
    captures::insert_input(conn, &capture.id, InputKind::File, Some(filename), text).map_err(db_err)?;
    captures::set_proposal(conn, &capture.id, &proposal, (0, 0)).map_err(db_err)?;
    let selected: Vec<usize> = proposal.ops.iter().map(|o| o.index).collect();
    let changes = apply_proposal(conn, &capture.id, &proposal, &selected).map_err(|e| e.to_string())?;
    Ok((capture.id, changes, proposal.warnings))
}

/// The same status change the detail panel's dropdown makes.
pub fn set_status(conn: &Connection, id: &str, status: &str) -> Result<Task, String> {
    let status = Status::from_str(status).map_err(|_| format!("Unknown status {status:?}; use todo, in_progress or done."))?;
    tasks::get_task(conn, id).map_err(|e| format!("Database error: {e}"))?.ok_or_else(|| format!("No task with id {id}"))?;
    tasks::update_task(conn, id, &TaskPatch { status: Some(status), ..Default::default() }).map_err(|e| format!("Database error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;
    use crate::models::{CaptureStatus, Status};
    use crate::repo::captures::get_capture;
    use crate::repo::tasks::{get_task, list_tasks};

    const TWO_CREATES: &str = r#"{"summary":"import","operations":[
      {"op":"create","temp_id":"t1","task_id":null,"title":"Parent work","description":"why","priority":"high","category":"Atlas","due_date":"2026-11-10","parent_id":null,"body":null,"reason":"from notes","evidence":null},
      {"op":"create","temp_id":"t2","task_id":null,"title":"Child step","description":null,"priority":null,"category":"Atlas","due_date":null,"parent_id":"t1","body":null,"reason":"sub-item","evidence":null}
    ]}"#;

    #[test]
    fn import_creates_tasks_and_records_capture() {
        let conn = open_memory().unwrap();
        let (capture_id, changes, warnings) = import_proposal(&conn, TWO_CREATES, "import-1.json", "manual-import").unwrap();
        assert_eq!(changes.len(), 2);
        assert!(warnings.is_empty(), "{warnings:?}");
        let tasks = list_tasks(&conn, true).unwrap();
        assert_eq!(tasks.len(), 2);
        let parent = tasks.iter().find(|t| t.title == "Parent work").unwrap();
        let child = tasks.iter().find(|t| t.title == "Child step").unwrap();
        assert_eq!(child.parent_id.as_deref(), Some(parent.id.as_str()));
        let cap = get_capture(&conn, &capture_id).unwrap().unwrap();
        assert_eq!(cap.status, CaptureStatus::Applied);
        assert_eq!(cap.model, "manual-import");
    }

    #[test]
    fn import_rejects_bad_json() {
        let conn = open_memory().unwrap();
        let err = import_proposal(&conn, "{nope", "x.json", "manual-import").unwrap_err();
        assert!(err.contains("could not be read"), "{err}");
        assert!(list_tasks(&conn, true).unwrap().is_empty());
    }

    #[test]
    fn set_status_updates_the_task() {
        let conn = open_memory().unwrap();
        let (_, changes, _) = import_proposal(&conn, TWO_CREATES, "x.json", "m").unwrap();
        let id = changes[0].task_id.clone().unwrap();
        let t = set_status(&conn, &id, "in_progress").unwrap();
        assert_eq!(t.status, Status::InProgress);
        assert_eq!(get_task(&conn, &id).unwrap().unwrap().status, Status::InProgress);
        assert!(set_status(&conn, &id, "later").is_err());
        assert!(set_status(&conn, "missing", "done").is_err());
    }

    #[test]
    fn maybe_run_ignores_normal_launch() {
        assert_eq!(maybe_run("com.parth.unpack", &["unpack.exe".to_string()]), None);
        assert_eq!(maybe_run("com.parth.unpack", &[]), None);
    }

    #[test]
    fn data_dir_is_under_appdata_with_the_identifier() {
        let d = data_dir("com.parth.unpack");
        assert!(d.ends_with("com.parth.unpack"), "{}", d.display());
    }
}
