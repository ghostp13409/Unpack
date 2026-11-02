//! `unpack --mcp`: a Model Context Protocol server over stdin/stdout.
//!
//! Claude Code (or any MCP client) starts the executable with `--mcp` and talks
//! JSON-RPC 2.0 to it, one message per line. The server opens the same database
//! as the window, so both see the same tasks; the window reloads its data when
//! it regains focus.
//!
//! Register it with Claude Code:
//!
//! ```text
//! claude mcp add --scope user unpack -- "%LOCALAPPDATA%\Unpack\unpack.exe" --mcp
//! ```

use crate::ai::ops::{ModelResponse, ProposedOp};
use crate::apply::{undo_change, UndoOutcome};
use crate::commands::{apply_selected, capture_summaries, lock_db, run_capture, task_detail};
use crate::headless::{data_dir, import_response, probe};
use crate::models::{CaptureStatus, NewTask, Priority, Status, Task, TaskPatch};
use crate::ranking::up_next;
use crate::repo::{captures, tasks};
use crate::{db, secrets, settings};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Mutex;

/// Protocol revisions this server can speak; the client's choice wins when listed.
const SUPPORTED_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const LATEST_VERSION: &str = SUPPORTED_VERSIONS[0];

const INSTRUCTIONS: &str = "Unpack is the user's personal task tracker. \
Use list_tasks or up_next to see the work, get_task for a task's notes, sub-tasks and history, \
and create_task, update_task and add_note to change it. Task ids are UUIDs; get them from list_tasks. \
To turn meeting notes, emails or documents into task changes, either read the material yourself and \
call apply_operations (the changes are recorded in Unpack's Activity, where each one can be undone), \
or call capture to let Unpack's own configured model propose operations and then apply_capture. \
Statuses: todo, in_progress, done, dropped (dropping hides a task). Priorities: urgent, high, medium, low. \
Dates are YYYY-MM-DD.";

const PREVIEW_CHARS: usize = 200;

pub struct Server {
    db: Mutex<Connection>,
    data_dir: PathBuf,
}

/// Opens the app's database and serves MCP on stdin/stdout until the client hangs up.
pub fn serve_stdio(identifier: &str) -> i32 {
    let dir = data_dir(identifier);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("Could not create the data folder: {e}");
        return 1;
    }
    let conn = match db::open(&dir.join("tasks.db")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Database error: {e}");
            return 1;
        }
    };
    let server = Server::new(conn, dir);
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    match server.serve(stdin.lock(), stdout.lock()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("MCP transport error: {e}");
            1
        }
    }
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn tool_text(payload: &Value, is_error: bool) -> Value {
    let text = match payload {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    json!({"content": [{"type": "text", "text": text}], "isError": is_error})
}

impl Server {
    pub fn new(conn: Connection, data_dir: PathBuf) -> Server {
        Server { db: Mutex::new(conn), data_dir }
    }

    /// Reads newline-delimited JSON-RPC messages and writes one reply line per request.
    pub fn serve<R: BufRead, W: Write>(&self, input: R, mut out: W) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let reply = match serde_json::from_str::<Value>(line) {
                Ok(Value::Array(batch)) => {
                    let replies: Vec<Value> = batch.iter().filter_map(|m| self.handle(m)).collect();
                    (!replies.is_empty()).then_some(Value::Array(replies))
                }
                Ok(msg) => self.handle(&msg),
                Err(e) => Some(rpc_error(Value::Null, -32700, &format!("Parse error: {e}"))),
            };
            if let Some(reply) = reply {
                writeln!(out, "{reply}")?;
                out.flush()?;
            }
        }
        Ok(())
    }

    /// Handles one message. Notifications and stray responses get no reply.
    pub fn handle(&self, msg: &Value) -> Option<Value> {
        let method = msg.get("method").and_then(Value::as_str);
        let id = msg.get("id").cloned();
        let (Some(method), Some(id)) = (method, id) else {
            // A notification (no id) or a response to a request we never send.
            return None;
        };
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        Some(match method {
            "initialize" => rpc_result(id, initialize(&params)),
            "ping" => rpc_result(id, json!({})),
            "tools/list" => rpc_result(id, json!({"tools": tool_definitions()})),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
                let empty = Map::new();
                let args = params.get("arguments").and_then(Value::as_object).unwrap_or(&empty);
                match self.call_tool(name, args) {
                    None => rpc_error(id, -32602, &format!("Unknown tool: {name}")),
                    Some(Ok(payload)) => rpc_result(id, tool_text(&payload, false)),
                    Some(Err(message)) => rpc_result(id, tool_text(&Value::String(message), true)),
                }
            }
            other => rpc_error(id, -32601, &format!("Method not found: {other}")),
        })
    }

    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        lock_db(&self.db)
    }

    /// Runs a tool. `None` means there is no tool by that name.
    fn call_tool(&self, name: &str, args: &Map<String, Value>) -> Option<Result<Value, String>> {
        let result = match name {
            "list_tasks" => self.list_tasks(args),
            "get_task" => self.get_task(args),
            "create_task" => self.create_task(args),
            "update_task" => self.update_task(args),
            "add_note" => self.add_note(args),
            "delete_note" => self.delete_note(args),
            "list_categories" => self.conn().and_then(|c| {
                tasks::list_categories(&c).map(|cats| json!({"categories": cats})).map_err(db_err)
            }),
            "up_next" => self.up_next(args),
            "capture" => self.capture(args),
            "apply_capture" => self.apply_capture(args),
            "discard_capture" => self.discard_capture(args),
            "apply_operations" => self.apply_operations(args),
            "list_activity" => self.list_activity(args),
            "undo_change" => self.undo(args),
            "get_settings" => self.get_settings(),
            "update_settings" => self.update_settings(args),
            "check_provider" => self.conn().and_then(|c| probe(&c)).map(|s| json!(s)),
            _ => return None,
        };
        Some(result)
    }

    fn list_tasks(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let include_done = opt_bool(args, "include_done")?.unwrap_or(false);
        let status = opt_str(args, "status")?.map(|s| parse_status(&s)).transpose()?;
        let category = opt_str(args, "category")?.map(|c| c.to_lowercase());
        let parent_id = opt_str(args, "parent_id")?;
        let query = opt_str(args, "query")?.map(|q| q.to_lowercase());
        let limit = opt_usize(args, "limit")?.unwrap_or(200);

        let want_closed = include_done || matches!(status, Some(Status::Done) | Some(Status::Dropped));
        let conn = self.conn()?;
        let mut all = tasks::list_tasks(&conn, want_closed).map_err(db_err)?;
        if status == Some(Status::Dropped) {
            all = dropped_tasks(&conn)?;
        }
        let matched: Vec<Task> = all
            .into_iter()
            .filter(|t| status.is_none_or(|s| t.status == s))
            .filter(|t| category.as_ref().is_none_or(|c| t.category.as_deref().map(str::to_lowercase).as_ref() == Some(c)))
            .filter(|t| parent_id.as_ref().is_none_or(|p| t.parent_id.as_ref() == Some(p)))
            .filter(|t| {
                query.as_ref().is_none_or(|q| t.title.to_lowercase().contains(q) || t.description.to_lowercase().contains(q))
            })
            .collect();
        let total = matched.len();
        let shown: Vec<Task> = matched.into_iter().take(limit).collect();
        Ok(json!({"total": total, "returned": shown.len(), "tasks": shown}))
    }

    fn get_task(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let id = req_str(args, "id")?;
        task_detail(&*self.conn()?, &id).map(|d| json!(d))
    }

    fn create_task(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let title = req_str(args, "title")?;
        let new = NewTask {
            title,
            description: opt_str(args, "description")?.unwrap_or_default(),
            priority: opt_str(args, "priority")?.map(|p| parse_priority(&p)).transpose()?.unwrap_or_default(),
            category: opt_str(args, "category")?,
            due_date: opt_str(args, "due_date")?.map(|d| check_date("due_date", d)).transpose()?,
            parent_id: opt_str(args, "parent_id")?,
        };
        let status = opt_str(args, "status")?.map(|s| parse_status(&s)).transpose()?;
        let conn = self.conn()?;
        if let Some(p) = &new.parent_id {
            require_task(&conn, p)?;
        }
        let task = tasks::insert_task(&conn, &new).map_err(db_err)?;
        let task = match status {
            Some(s) if s != Status::Todo => {
                tasks::update_task(&conn, &task.id, &TaskPatch { status: Some(s), ..Default::default() }).map_err(db_err)?
            }
            _ => task,
        };
        Ok(json!(task))
    }

    fn update_task(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let id = req_str(args, "id")?;
        let title = opt_str(args, "title")?;
        if args.contains_key("title") && title.is_none() {
            return Err("title cannot be empty.".to_string());
        }
        let patch = TaskPatch {
            title,
            description: match args.get("description") {
                None => None,
                Some(Value::Null) => Some(String::new()),
                Some(Value::String(s)) => Some(s.trim().to_string()),
                Some(_) => return Err("description must be a string.".to_string()),
            },
            status: opt_str(args, "status")?.map(|s| parse_status(&s)).transpose()?,
            priority: opt_str(args, "priority")?.map(|p| parse_priority(&p)).transpose()?,
            category: nullable_str(args, "category")?,
            due_date: nullable_date(args, "due_date")?,
            parent_id: nullable_str(args, "parent_id")?,
            pinned: opt_bool(args, "pinned")?,
            snoozed_until: nullable_date(args, "snoozed_until")?,
        };
        if patch == TaskPatch::default() {
            return Err("Nothing to change: give at least one field besides id.".to_string());
        }
        let conn = self.conn()?;
        require_task(&conn, &id)?;
        if let Some(Some(p)) = &patch.parent_id {
            if *p == id {
                return Err("A task cannot be its own parent.".to_string());
            }
            require_task(&conn, p)?;
        }
        tasks::update_task(&conn, &id, &patch).map(|t| json!(t)).map_err(db_err)
    }

    fn add_note(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let task_id = req_str(args, "task_id")?;
        let body = req_str(args, "body")?;
        let conn = self.conn()?;
        require_task(&conn, &task_id)?;
        tasks::add_note(&conn, &task_id, &body, None).map(|n| json!(n)).map_err(db_err)
    }

    fn delete_note(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let note_id = req_str(args, "note_id")?;
        let conn = self.conn()?;
        let exists: Option<String> = conn
            .query_row("SELECT id FROM notes WHERE id = ?1", [&note_id], |r| r.get(0))
            .optional()
            .map_err(db_err)?;
        if exists.is_none() {
            return Err(format!("No note with id {note_id}"));
        }
        tasks::delete_note(&conn, &note_id).map_err(db_err)?;
        Ok(json!({"deleted": note_id}))
    }

    fn up_next(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let limit = opt_usize(args, "limit")?.unwrap_or(5).min(50);
        let all = tasks::list_tasks(&*self.conn()?, true).map_err(db_err)?;
        let today = chrono::Local::now().date_naive();
        let by_id: HashMap<&str, &Task> = all.iter().map(|t| (t.id.as_str(), t)).collect();
        let ranked: Vec<Value> = up_next(&all, today, limit)
            .into_iter()
            .filter_map(|r| {
                let t = by_id.get(r.task_id.as_str())?;
                Some(json!({
                    "task_id": r.task_id, "title": t.title, "status": t.status, "priority": t.priority,
                    "category": t.category, "due_date": t.due_date, "pinned": t.pinned,
                    "score": r.score, "reasons": r.reasons
                }))
            })
            .collect();
        Ok(json!({"today": today.format("%Y-%m-%d").to_string(), "up_next": ranked}))
    }

    fn capture(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let text = opt_str(args, "text")?;
        let paths = opt_str_list(args, "paths")?;
        let capture = tauri::async_runtime::block_on(run_capture(&self.db, &self.data_dir, text, paths))?;
        Ok(json!({
            "capture": capture,
            "next": "Nothing is applied yet. Review proposal.ops, then call apply_capture with the indexes to keep (or all), or discard_capture."
        }))
    }

    fn apply_capture(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let capture_id = req_str(args, "capture_id")?;
        let conn = self.conn()?;
        let selected = match opt_usize_list(args, "selected")? {
            Some(s) => s,
            None => {
                let capture = captures::get_capture(&conn, &capture_id).map_err(db_err)?.ok_or("Capture not found")?;
                capture.proposal.map(|p| p.ops.iter().map(|o: &ProposedOp| o.index).collect()).unwrap_or_default()
            }
        };
        let changes = apply_selected(&conn, &capture_id, &selected)?;
        Ok(json!({"applied": changes.len(), "changes": changes}))
    }

    fn discard_capture(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let capture_id = req_str(args, "capture_id")?;
        let conn = self.conn()?;
        let capture = captures::get_capture(&conn, &capture_id).map_err(db_err)?.ok_or("Capture not found")?;
        if capture.status != CaptureStatus::Proposed {
            return Err(format!("This capture is already {}; only a proposed capture can be discarded.", capture.status.as_str()));
        }
        captures::set_status(&conn, &capture_id, CaptureStatus::Discarded).map_err(db_err)?;
        Ok(json!({"discarded": capture_id}))
    }

    fn apply_operations(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let summary = opt_str(args, "summary")?.unwrap_or_else(|| "Changes from Claude Code".to_string());
        let label = opt_str(args, "label")?.unwrap_or_else(|| "claude-code-mcp".to_string());
        let source = opt_str(args, "source")?.unwrap_or_else(|| "Claude Code".to_string());
        let ops = match args.get("operations") {
            Some(Value::Array(ops)) if !ops.is_empty() => ops,
            _ => return Err("operations must be a non-empty array.".to_string()),
        };
        let mut raw = Vec::with_capacity(ops.len());
        for (i, op) in ops.iter().enumerate() {
            let mut op = op.as_object().cloned().ok_or_else(|| format!("operations[{i}] must be an object."))?;
            op.entry("reason").or_insert_with(|| json!(""));
            raw.push(Value::Object(op));
        }
        let response = json!({"summary": summary, "operations": raw});
        let resp: ModelResponse =
            serde_json::from_value(response.clone()).map_err(|e| format!("The operations could not be read: {e}"))?;
        let text = serde_json::to_string_pretty(&response).unwrap_or_default();
        let (capture_id, changes, warnings) = import_response(&*self.conn()?, resp, &text, &source, &label)?;
        Ok(json!({"capture_id": capture_id, "applied": changes.len(), "changes": changes, "warnings": warnings}))
    }

    fn list_activity(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let limit = opt_usize(args, "limit")?.unwrap_or(10).min(100);
        let full_text = opt_bool(args, "include_input_text")?.unwrap_or(false);
        let mut out = json!(capture_summaries(&*self.conn()?, limit)?);
        if !full_text {
            for summary in out.as_array_mut().into_iter().flatten() {
                for input in summary["inputs"].as_array_mut().into_iter().flatten() {
                    if let Some(text) = input["extracted_text"].as_str() {
                        input["extracted_text"] = json!(preview(text));
                    }
                }
            }
        }
        Ok(json!({"captures": out}))
    }

    fn undo(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let change_id = req_str(args, "change_id")?;
        let force = opt_bool(args, "force")?.unwrap_or(false);
        let outcome = undo_change(&*self.conn()?, &change_id, force).map_err(|e| e.to_string())?;
        Ok(match &outcome {
            UndoOutcome::Done => json!({"outcome": outcome}),
            UndoOutcome::NeedsConfirm { changed_fields } => json!({
                "outcome": outcome,
                "message": format!(
                    "Not undone: {} changed again since this change. Call undo_change with force: true to overwrite.",
                    changed_fields.join(", ")
                )
            }),
        })
    }

    fn get_settings(&self) -> Result<Value, String> {
        let conn = self.conn()?;
        let has_key = secrets::get_api_key().unwrap_or(None).is_some();
        Ok(json!({
            "settings": settings::load(&conn),
            "has_api_key": has_key,
            "provider_status": settings::load_status(&conn),
            "data_dir": self.data_dir.to_string_lossy(),
        }))
    }

    fn update_settings(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let conn = self.conn()?;
        let mut s = settings::load(&conn);
        let before = s.clone();
        if let Some(v) = opt_str(args, "model")? {
            s.model = v;
        }
        if let Some(v) = opt_str(args, "effort")? {
            s.effort = one_of("effort", v, &["low", "medium", "high"])?;
        }
        if let Some(v) = opt_str(args, "theme")? {
            s.theme = one_of("theme", v, &["system", "light", "dark"])?;
        }
        if let Some(v) = opt_str(args, "provider")? {
            s.provider = one_of("provider", v, &["claude_code", "api_key"])?;
        }
        for (key, field) in [("user_name", &mut s.user_name), ("manager_name", &mut s.manager_name), ("claude_code_path", &mut s.claude_code_path)] {
            match args.get(key) {
                None => {}
                Some(Value::Null) => field.clear(),
                Some(Value::String(v)) => *field = v.trim().to_string(),
                Some(_) => return Err(format!("{key} must be a string.")),
            }
        }
        if s == before {
            return Err("Nothing to change: give at least one setting.".to_string());
        }
        settings::save(&conn, &s).map_err(db_err)?;
        let mut out = json!({"settings": s});
        if s.provider != before.provider || s.claude_code_path != before.claude_code_path {
            out["next"] = json!("The provider changed; call check_provider to refresh its readiness.");
        }
        Ok(out)
    }
}

fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(LATEST_VERSION);
    let version = if SUPPORTED_VERSIONS.contains(&asked) { asked } else { LATEST_VERSION };
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {"listChanged": false}},
        "serverInfo": {"name": "unpack", "title": "Unpack", "version": env!("CARGO_PKG_VERSION")},
        "instructions": INSTRUCTIONS,
    })
}

fn db_err(e: rusqlite::Error) -> String {
    format!("Database error: {e}")
}

fn preview(text: &str) -> String {
    if text.chars().count() <= PREVIEW_CHARS {
        return text.to_string();
    }
    let head: String = text.chars().take(PREVIEW_CHARS).collect();
    format!("{head}… (shortened; pass include_input_text: true for all of it)")
}

fn require_task(conn: &Connection, id: &str) -> Result<Task, String> {
    tasks::get_task(conn, id).map_err(db_err)?.ok_or_else(|| format!("No task with id {id}"))
}

fn dropped_tasks(conn: &Connection) -> Result<Vec<Task>, String> {
    let mut stmt = conn
        .prepare("SELECT id FROM tasks WHERE status = 'dropped' ORDER BY created_at, id")
        .map_err(db_err)?;
    let ids: Vec<String> = stmt.query_map([], |r| r.get(0)).map_err(db_err)?.collect::<Result<_, _>>().map_err(db_err)?;
    ids.iter().filter_map(|id| tasks::get_task(conn, id).map_err(db_err).transpose()).collect()
}

fn parse_status(s: &str) -> Result<Status, String> {
    Status::from_str(s).map_err(|_| format!("Unknown status {s:?}; use todo, in_progress, done or dropped."))
}

fn parse_priority(s: &str) -> Result<Priority, String> {
    Priority::from_str(s).map_err(|_| format!("Unknown priority {s:?}; use urgent, high, medium or low."))
}

fn check_date(key: &str, d: String) -> Result<String, String> {
    chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d")
        .map(|_| d.clone())
        .map_err(|_| format!("{key} must be a date like 2026-11-30, not {d:?}."))
}

fn one_of(key: &str, v: String, allowed: &[&str]) -> Result<String, String> {
    if allowed.contains(&v.as_str()) {
        Ok(v)
    } else {
        Err(format!("{key} must be one of {}.", allowed.join(", ")))
    }
}

/// A trimmed string argument; absent, null and blank all read as `None`.
fn opt_str(args: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            let t = s.trim();
            Ok((!t.is_empty()).then(|| t.to_string()))
        }
        Some(_) => Err(format!("{key} must be a string.")),
    }
}

fn req_str(args: &Map<String, Value>, key: &str) -> Result<String, String> {
    opt_str(args, key)?.ok_or_else(|| format!("{key} is required."))
}

/// For fields that can be cleared: absent leaves the value, null or "" clears it.
fn nullable_str(args: &Map<String, Value>, key: &str) -> Result<Option<Option<String>>, String> {
    match args.get(key) {
        None => Ok(None),
        Some(_) => opt_str(args, key).map(Some),
    }
}

fn nullable_date(args: &Map<String, Value>, key: &str) -> Result<Option<Option<String>>, String> {
    match nullable_str(args, key)? {
        Some(Some(d)) => Ok(Some(Some(check_date(key, d)?))),
        other => Ok(other),
    }
}

fn opt_bool(args: &Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("{key} must be true or false.")),
    }
}

fn opt_usize(args: &Map<String, Value>, key: &str) -> Result<Option<usize>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_u64().map(|n| Some(n as usize)).ok_or_else(|| format!("{key} must be a whole number.")),
    }
}

fn opt_usize_list(args: &Map<String, Value>, key: &str) -> Result<Option<Vec<usize>>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| v.as_u64().map(|n| n as usize).ok_or_else(|| format!("{key} must hold whole numbers.")))
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(format!("{key} must be an array of whole numbers.")),
    }
}

fn opt_str_list(args: &Map<String, Value>, key: &str) -> Result<Vec<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| v.as_str().map(str::to_string).ok_or_else(|| format!("{key} must hold strings.")))
            .collect(),
        Some(_) => Err(format!("{key} must be an array of strings.")),
    }
}

fn tool(name: &str, title: &str, description: &str, properties: Value, required: &[&str], read_only: bool) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": {"type": "object", "properties": properties, "required": required},
        "annotations": {"readOnlyHint": read_only, "destructiveHint": false, "openWorldHint": false},
    })
}

/// Every tool the server offers, with JSON Schemas for their arguments.
pub fn tool_definitions() -> Vec<Value> {
    let status = json!({"type": "string", "enum": ["todo", "in_progress", "done", "dropped"]});
    let priority = json!({"type": "string", "enum": ["urgent", "high", "medium", "low"]});
    let date = json!({"type": "string", "description": "YYYY-MM-DD"});
    let clearable = |desc: &str| json!({"type": ["string", "null"], "description": desc});
    vec![
        tool(
            "list_tasks",
            "List tasks",
            "Lists tasks with optional filters. By default only open tasks (todo and in_progress); done tasks need include_done or status: done. Dropped tasks appear only with status: dropped.",
            json!({
                "include_done": {"type": "boolean", "description": "Also list done tasks. Default false."},
                "status": status,
                "category": {"type": "string", "description": "Exact category name, any case."},
                "parent_id": {"type": "string", "description": "Only sub-tasks of this task."},
                "query": {"type": "string", "description": "Text to find in the title or description, any case."},
                "limit": {"type": "integer", "minimum": 1, "description": "Most tasks to return. Default 200."}
            }),
            &[],
            true,
        ),
        tool(
            "get_task",
            "Get a task",
            "One task with its notes, sub-tasks, and the captures that created or changed it.",
            json!({"id": {"type": "string"}}),
            &["id"],
            true,
        ),
        tool(
            "create_task",
            "Create a task",
            "Adds a task. New tasks start as todo unless status says otherwise.",
            json!({
                "title": {"type": "string"},
                "description": {"type": "string"},
                "priority": priority,
                "category": {"type": "string"},
                "due_date": date,
                "parent_id": {"type": "string", "description": "Make it a sub-task of this task."},
                "status": status
            }),
            &["title"],
            false,
        ),
        tool(
            "update_task",
            "Update a task",
            "Changes the given fields of a task and leaves the rest alone. Pass null to clear category, due_date, parent_id or snoozed_until. Status done marks it complete; dropped removes it from every list.",
            json!({
                "id": {"type": "string"},
                "title": {"type": "string"},
                "description": {"type": "string"},
                "status": status,
                "priority": priority,
                "category": clearable("Category name, or null to clear."),
                "due_date": clearable("YYYY-MM-DD, or null to clear."),
                "parent_id": clearable("Parent task id, or null to make it top level."),
                "pinned": {"type": "boolean", "description": "Pinned tasks rank first in up_next."},
                "snoozed_until": clearable("Hide from up_next until this YYYY-MM-DD, or null to unsnooze.")
            }),
            &["id"],
            false,
        ),
        tool(
            "add_note",
            "Add a note",
            "Adds a dated note to a task, such as a progress update or a decision.",
            json!({"task_id": {"type": "string"}, "body": {"type": "string"}}),
            &["task_id", "body"],
            false,
        ),
        tool(
            "delete_note",
            "Delete a note",
            "Deletes one note by its id (see get_task).",
            json!({"note_id": {"type": "string"}}),
            &["note_id"],
            false,
        ),
        tool("list_categories", "List categories", "The category names in use.", json!({}), &[], true),
        tool(
            "up_next",
            "Up next",
            "The open tasks to work on next, ranked the way Unpack's Focus view ranks them (pinned, priority, due date, progress), with the reasons for each score.",
            json!({"limit": {"type": "integer", "minimum": 1, "maximum": 50, "description": "Default 5."}}),
            &[],
            true,
        ),
        tool(
            "capture",
            "Capture with Unpack's model",
            "Sends text and/or files (.vtt, .eml, .docx, .pdf, .md, .txt, .html) to Unpack's configured model, which proposes task operations. Nothing changes until apply_capture. Slow: up to five minutes. To do the reading yourself, use apply_operations instead.",
            json!({
                "text": {"type": "string", "description": "Pasted text such as notes or an email."},
                "paths": {"type": "array", "items": {"type": "string"}, "description": "Absolute file paths."}
            }),
            &[],
            false,
        ),
        tool(
            "apply_capture",
            "Apply a capture",
            "Applies a proposed capture's operations. selected lists the op indexes to keep; omit it to apply all of them.",
            json!({
                "capture_id": {"type": "string"},
                "selected": {"type": "array", "items": {"type": "integer", "minimum": 0}}
            }),
            &["capture_id"],
            false,
        ),
        tool(
            "discard_capture",
            "Discard a capture",
            "Discards a proposed capture without changing any task.",
            json!({"capture_id": {"type": "string"}}),
            &["capture_id"],
            false,
        ),
        tool(
            "apply_operations",
            "Apply task operations",
            "Applies a batch of task operations you worked out yourself (for example from a meeting transcript) and records them as one capture in Unpack's Activity, so the user can review and undo each change. Ops: create (title; optional description, priority, category, due_date, parent_id as an existing task id or another create's temp_id), update (task_id plus fields to change), complete (task_id), add_note (task_id, body). update, complete and add_note work on open tasks only. Give each a short reason and, when there is one, a quote as evidence.",
            json!({
                "summary": {"type": "string", "description": "One line on what these changes are."},
                "source": {"type": "string", "description": "Where they came from, such as a file name. Default \"Claude Code\"."},
                "operations": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "op": {"type": "string", "enum": ["create", "update", "complete", "add_note"]},
                            "temp_id": {"type": "string", "description": "create only: a label other creates can use as parent_id."},
                            "task_id": {"type": "string", "description": "update, complete, add_note: the existing task."},
                            "title": {"type": "string"},
                            "description": {"type": "string"},
                            "priority": priority,
                            "category": {"type": "string"},
                            "due_date": date,
                            "parent_id": {"type": "string"},
                            "body": {"type": "string", "description": "add_note: the note text."},
                            "reason": {"type": "string"},
                            "evidence": {"type": "string"}
                        },
                        "required": ["op"]
                    }
                },
                "label": {"type": "string", "description": "Shown as the capture's model. Default claude-code-mcp."}
            }),
            &["operations"],
            false,
        ),
        tool(
            "list_activity",
            "List activity",
            "Recent captures, newest first, with their inputs and the changes they made. Change ids from here go to undo_change.",
            json!({
                "limit": {"type": "integer", "minimum": 1, "maximum": 100, "description": "Default 10."},
                "include_input_text": {"type": "boolean", "description": "Return the full input text instead of a short preview."}
            }),
            &[],
            true,
        ),
        tool(
            "undo_change",
            "Undo a change",
            "Reverts one applied change (see list_activity or get_task). If the task was edited again since, it reports the fields and changes nothing unless force is true.",
            json!({"change_id": {"type": "string"}, "force": {"type": "boolean"}}),
            &["change_id"],
            false,
        ),
        tool(
            "get_settings",
            "Get settings",
            "Unpack's settings (model, effort, names, theme, provider) and the provider's last readiness check.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "update_settings",
            "Update settings",
            "Changes the given settings. The API key cannot be set here; use the Settings view in the app.",
            json!({
                "model": {"type": "string"},
                "effort": {"type": "string", "enum": ["low", "medium", "high"]},
                "user_name": {"type": "string"},
                "manager_name": {"type": "string"},
                "theme": {"type": "string", "enum": ["system", "light", "dark"]},
                "provider": {"type": "string", "enum": ["claude_code", "api_key"]},
                "claude_code_path": {"type": "string"}
            }),
            &[],
            false,
        ),
        tool(
            "check_provider",
            "Check the provider",
            "Re-checks whether Unpack's model provider (Claude Code sign-in or API key) is ready, and saves the result.",
            json!({}),
            &[],
            false,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ops::resolve;
    use crate::db::open_memory;

    fn server() -> Server {
        Server::new(open_memory().unwrap(), PathBuf::from("."))
    }

    fn request(server: &Server, method: &str, params: Value) -> Value {
        server.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})).unwrap()
    }

    /// Calls a tool; returns the parsed payload and whether it was an error.
    fn call(server: &Server, name: &str, args: Value) -> (Value, bool) {
        let reply = request(server, "tools/call", json!({"name": name, "arguments": args}));
        let result = &reply["result"];
        let text = result["content"][0]["text"].as_str().unwrap().to_string();
        let is_error = result["isError"].as_bool().unwrap();
        (serde_json::from_str(&text).unwrap_or(Value::String(text)), is_error)
    }

    fn ok(server: &Server, name: &str, args: Value) -> Value {
        let (v, is_error) = call(server, name, args);
        assert!(!is_error, "{name} failed: {v}");
        v
    }

    #[test]
    fn initialize_echoes_a_known_version_and_offers_tools() {
        let s = server();
        let r = request(&s, "initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}));
        assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
        assert!(r["result"]["capabilities"]["tools"].is_object());
        assert_eq!(r["result"]["serverInfo"]["name"], "unpack");
        let r = request(&s, "initialize", json!({"protocolVersion": "1999-01-01"}));
        assert_eq!(r["result"]["protocolVersion"], LATEST_VERSION);
    }

    #[test]
    fn notifications_get_no_reply_and_unknown_methods_error() {
        let s = server();
        assert!(s.handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none());
        assert!(s.handle(&json!({"jsonrpc": "2.0", "id": 4, "result": {}})).is_none());
        let r = request(&s, "resources/list", json!({}));
        assert_eq!(r["error"]["code"], -32601);
        let r = request(&s, "tools/call", json!({"name": "nope", "arguments": {}}));
        assert_eq!(r["error"]["code"], -32602);
        assert_eq!(request(&s, "ping", Value::Null)["result"], json!({}));
    }

    #[test]
    fn tools_list_names_every_tool_with_an_object_schema() {
        let s = server();
        let r = request(&s, "tools/list", json!({}));
        let tools = r["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for want in [
            "list_tasks", "get_task", "create_task", "update_task", "add_note", "delete_note", "list_categories",
            "up_next", "capture", "apply_capture", "discard_capture", "apply_operations", "list_activity",
            "undo_change", "get_settings", "update_settings", "check_provider",
        ] {
            assert!(names.contains(&want), "missing {want}");
        }
        for t in tools {
            assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
            // Every listed tool is routed (an unrouted one would answer "Unknown tool").
            let name = t["name"].as_str().unwrap();
            if !["capture", "check_provider"].contains(&name) {
                let r = request(&s, "tools/call", json!({"name": name, "arguments": {}}));
                assert!(r.get("result").is_some(), "{name} is not routed");
            }
        }
    }

    #[test]
    fn create_list_update_and_get_roundtrip() {
        let s = server();
        let parent = ok(&s, "create_task", json!({"title": "Plan Atlas launch", "priority": "high", "category": "Platform", "due_date": "2026-11-20"}));
        let pid = parent["id"].as_str().unwrap().to_string();
        let child = ok(&s, "create_task", json!({"title": "Draft runbook", "parent_id": pid, "status": "in_progress"}));
        assert_eq!(child["status"], "in_progress");

        let list = ok(&s, "list_tasks", json!({}));
        assert_eq!(list["total"], 2);
        let list = ok(&s, "list_tasks", json!({"category": "platform"}));
        assert_eq!(list["total"], 1);
        let list = ok(&s, "list_tasks", json!({"query": "RUNBOOK"}));
        assert_eq!(list["tasks"][0]["title"], "Draft runbook");
        let list = ok(&s, "list_tasks", json!({"parent_id": pid}));
        assert_eq!(list["total"], 1);

        let updated = ok(&s, "update_task", json!({"id": pid, "due_date": null, "pinned": true, "category": "Observability"}));
        assert_eq!(updated["due_date"], Value::Null);
        assert_eq!(updated["pinned"], true);
        assert_eq!(updated["category"], "Observability");

        let done = ok(&s, "update_task", json!({"id": child["id"], "status": "done"}));
        assert!(done["completed_at"].is_string());
        assert_eq!(ok(&s, "list_tasks", json!({}))["total"], 1);
        assert_eq!(ok(&s, "list_tasks", json!({"include_done": true}))["total"], 2);
        assert_eq!(ok(&s, "list_tasks", json!({"status": "done"}))["total"], 1);

        let note = ok(&s, "add_note", json!({"task_id": pid, "body": "Alex Morgan signed off"}));
        let detail = ok(&s, "get_task", json!({"id": pid}));
        assert_eq!(detail["notes"][0]["body"], "Alex Morgan signed off");
        assert_eq!(detail["children"].as_array().unwrap().len(), 1);
        ok(&s, "delete_note", json!({"note_id": note["id"]}));
        assert!(ok(&s, "get_task", json!({"id": pid}))["notes"].as_array().unwrap().is_empty());

        assert_eq!(ok(&s, "list_categories", json!({}))["categories"], json!(["Observability"]));
        let next = ok(&s, "up_next", json!({}));
        assert_eq!(next["up_next"][0]["title"], "Plan Atlas launch");
        assert!(next["up_next"][0]["reasons"].as_array().unwrap().iter().any(|r| r == "Pinned"));

        ok(&s, "update_task", json!({"id": pid, "status": "dropped"}));
        assert_eq!(ok(&s, "list_tasks", json!({"status": "dropped"}))["total"], 1);
    }

    #[test]
    fn bad_arguments_come_back_as_tool_errors() {
        let s = server();
        let (msg, err) = call(&s, "create_task", json!({"title": "  "}));
        assert!(err && msg.as_str().unwrap().contains("title is required"));
        let (msg, err) = call(&s, "create_task", json!({"title": "x", "due_date": "next week"}));
        assert!(err && msg.as_str().unwrap().contains("due_date"));
        let (msg, err) = call(&s, "create_task", json!({"title": "x", "priority": "critical"}));
        assert!(err && msg.as_str().unwrap().contains("Unknown priority"));
        let (_, err) = call(&s, "create_task", json!({"title": "x", "parent_id": "missing"}));
        assert!(err);
        let t = ok(&s, "create_task", json!({"title": "x"}));
        let (msg, err) = call(&s, "update_task", json!({"id": t["id"]}));
        assert!(err && msg.as_str().unwrap().contains("Nothing to change"));
        let (_, err) = call(&s, "update_task", json!({"id": t["id"], "parent_id": t["id"]}));
        assert!(err);
        let (_, err) = call(&s, "update_task", json!({"id": "missing", "title": "y"}));
        assert!(err);
        let (_, err) = call(&s, "add_note", json!({"task_id": "missing", "body": "y"}));
        assert!(err);
        let (_, err) = call(&s, "delete_note", json!({"note_id": "missing"}));
        assert!(err);
    }

    #[test]
    fn apply_operations_records_a_capture_that_can_be_undone() {
        let s = server();
        let existing = ok(&s, "create_task", json!({"title": "Beacon pricing review"}));
        let r = ok(&s, "apply_operations", json!({
            "summary": "From the Beacon sync",
            "source": "beacon-sync.vtt",
            "operations": [
                {"op": "create", "temp_id": "t1", "title": "Beacon rollout", "priority": "urgent", "category": "Pricing"},
                {"op": "create", "title": "Email Jordan the numbers", "parent_id": "t1", "reason": "Sam asked"},
                {"op": "add_note", "task_id": existing["id"], "body": "Moved to Friday"},
                {"op": "complete", "task_id": existing["id"], "evidence": "that's done"}
            ]
        }));
        assert_eq!(r["applied"], 4, "{r}");
        let all = ok(&s, "list_tasks", json!({"include_done": true}));
        assert_eq!(all["total"], 3);
        let rollout = all["tasks"].as_array().unwrap().iter().find(|t| t["title"] == "Beacon rollout").unwrap().clone();
        let child = all["tasks"].as_array().unwrap().iter().find(|t| t["title"] == "Email Jordan the numbers").unwrap().clone();
        assert_eq!(child["parent_id"], rollout["id"]);

        let activity = ok(&s, "list_activity", json!({}));
        assert_eq!(activity["captures"][0]["capture"]["summary"], "From the Beacon sync");
        assert_eq!(activity["captures"][0]["inputs"][0]["filename"], "beacon-sync.vtt");

        let complete = r["changes"].as_array().unwrap().iter().find(|c| c["kind"] == "complete").unwrap();
        let undone = ok(&s, "undo_change", json!({"change_id": complete["id"]}));
        assert_eq!(undone["outcome"]["type"], "done");
        assert_eq!(ok(&s, "get_task", json!({"id": existing["id"]}))["task"]["status"], "todo");

        let (_, err) = call(&s, "apply_operations", json!({"operations": []}));
        assert!(err);
    }

    #[test]
    fn apply_and_discard_a_proposed_capture() {
        let s = server();
        let proposal = {
            let resp: ModelResponse = serde_json::from_value(json!({"summary": "two ideas", "operations": [
                {"op": "create", "title": "Keep me", "reason": ""},
                {"op": "create", "title": "Skip me", "reason": ""}
            ]}))
            .unwrap();
            resolve(resp, &HashMap::new())
        };
        let (first, second) = {
            let conn = s.conn().unwrap();
            let a = captures::insert_capture(&conn, "m").unwrap();
            captures::set_proposal(&conn, &a.id, &proposal, (0, 0)).unwrap();
            let b = captures::insert_capture(&conn, "m").unwrap();
            captures::set_proposal(&conn, &b.id, &proposal, (0, 0)).unwrap();
            (a.id, b.id)
        };
        let r = ok(&s, "apply_capture", json!({"capture_id": first, "selected": [0]}));
        assert_eq!(r["applied"], 1);
        let titles: Vec<Value> = ok(&s, "list_tasks", json!({}))["tasks"].as_array().unwrap().iter().map(|t| t["title"].clone()).collect();
        assert_eq!(titles, vec![json!("Keep me")]);
        let (_, err) = call(&s, "discard_capture", json!({"capture_id": first}));
        assert!(err, "an applied capture cannot be discarded");
        ok(&s, "discard_capture", json!({"capture_id": second}));
        let (_, err) = call(&s, "apply_capture", json!({"capture_id": second}));
        assert!(err, "a discarded capture cannot be applied");
    }

    #[test]
    fn settings_update_validates_and_saves() {
        let s = server();
        let r = ok(&s, "update_settings", json!({"effort": "high", "user_name": "Sam", "provider": "api_key"}));
        assert_eq!(r["settings"]["effort"], "high");
        assert!(r["next"].is_string());
        let got = ok(&s, "get_settings", json!({}));
        assert_eq!(got["settings"]["user_name"], "Sam");
        let (_, err) = call(&s, "update_settings", json!({"theme": "neon"}));
        assert!(err);
        let (_, err) = call(&s, "update_settings", json!({}));
        assert!(err);
    }

    #[test]
    fn serve_answers_each_request_line() {
        let s = server();
        let input = concat!(
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\"}}\n",
            "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
            "\n",
            "not json\n",
            "{\"jsonrpc\":\"2.0\",\"id\":\"b\",\"method\":\"tools/list\"}\n",
        );
        let mut out = Vec::new();
        s.serve(input.as_bytes(), &mut out).unwrap();
        let lines: Vec<Value> = String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["id"], 1);
        assert_eq!(lines[1]["error"]["code"], -32700);
        assert_eq!(lines[2]["id"], "b");
    }
}
