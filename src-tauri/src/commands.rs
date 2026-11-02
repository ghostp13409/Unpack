use crate::ai::claude_code::{self, CcError, CcRun};
use crate::ai::client::{run_extraction, test_connection as ai_test_connection, AiConfig, AiError, DEFAULT_BASE_URL};
use crate::ai::ops::resolve;
use crate::ai::prompt::ExtractionRequest;
use crate::apply::{apply_proposal, undo_change as apply_undo, UndoOutcome};
use crate::ingest::{extract_file, total_chars, Extracted, MAX_TOTAL_CHARS};
use crate::models::*;
use crate::ranking::{up_next as rank_up_next, Ranked};
use crate::repo::{captures, tasks};
use crate::secrets;
use crate::settings::{self, ProviderStatus, Settings};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub data_dir: PathBuf,
    /// Last readiness result of the selected provider (also persisted in settings).
    pub status: Mutex<ProviderStatus>,
}

const CC_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    ClaudeCode,
    ApiKey,
}

impl Provider {
    pub fn parse(s: &str) -> Option<Provider> {
        match s {
            "claude_code" => Some(Provider::ClaudeCode),
            "api_key" => Some(Provider::ApiKey),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::ClaudeCode => "claude_code",
            Provider::ApiKey => "api_key",
        }
    }
}

/// An unset provider picks Claude Code when it is ready, else the API key; a set one stays.
pub fn choose_provider(current: &str, cc_ready: bool) -> &'static str {
    match Provider::parse(current) {
        Some(p) => p.as_str(),
        None if cc_ready => "claude_code",
        None => "api_key",
    }
}

/// Finds Claude Code and checks how it is signed in. Ok carries the "Signed in as …" line.
async fn probe_claude_code(path_override: &str) -> Result<String, CcError> {
    let exe = claude_code::locate(path_override).ok_or(CcError::NotFound)?;
    let st = claude_code::auth_status(&exe).await?;
    claude_code::readiness(&st)
}

/// Readiness of the provider the settings select (or would select, when unset).
/// Takes a snapshot of the settings so no database lock is held across the probe.
pub async fn compute_status(settings: &Settings, has_key: bool) -> ProviderStatus {
    let cc = if Provider::parse(&settings.provider) == Some(Provider::ApiKey) {
        Err(CcError::NotFound)
    } else {
        probe_claude_code(&settings.claude_code_path).await
    };
    let provider = choose_provider(&settings.provider, cc.is_ok());
    let (ready, detail, account) = match Provider::parse(provider) {
        Some(Provider::ClaudeCode) => match cc {
            Ok(account) => (true, String::new(), account),
            Err(e) => (false, e.to_string(), String::new()),
        },
        _ => {
            if has_key {
                (true, "Key saved".to_string(), String::new())
            } else {
                (false, AiError::NoKey.to_string(), String::new())
            }
        }
    };
    ProviderStatus { provider: provider.to_string(), ready, detail, account, checked_at: now_iso() }
}

fn cc_run(settings: &Settings, data_dir: &Path) -> CmdResult<CcRun> {
    let exe = claude_code::locate(&settings.claude_code_path).ok_or_else(|| CcError::NotFound.to_string())?;
    Ok(CcRun {
        exe,
        model: settings.model.clone(),
        effort: settings.effort.clone(),
        scratch_dir: data_dir.join("tmp"),
        timeout: CC_TIMEOUT,
    })
}

type CmdResult<T> = Result<T, String>;

fn db_err(e: rusqlite::Error) -> String {
    format!("Database error: {e}")
}

fn lock<'a>(state: &'a State<'_, AppState>) -> CmdResult<std::sync::MutexGuard<'a, Connection>> {
    state.db.lock().map_err(|_| "The database is busy. Try again.".to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceRef {
    pub capture_id: String,
    pub captured_at: String,
    pub filenames: Vec<String>,
    pub kind: ChangeKind,
    pub reason: String,
    pub evidence: Option<String>,
    pub undone: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskDetail {
    pub task: Task,
    pub notes: Vec<Note>,
    pub children: Vec<Task>,
    pub sources: Vec<SourceRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureSummary {
    pub capture: Capture,
    pub inputs: Vec<CaptureInput>,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    pub settings: Settings,
    pub has_key: bool,
    pub key_hint: Option<String>,
    pub data_dir: String,
    pub provider_status: ProviderStatus,
}

#[tauri::command]
pub fn list_tasks(state: State<'_, AppState>, include_done: bool) -> CmdResult<Vec<Task>> {
    tasks::list_tasks(&*lock(&state)?, include_done).map_err(db_err)
}

#[tauri::command]
pub fn get_task_detail(state: State<'_, AppState>, id: String) -> CmdResult<TaskDetail> {
    let conn = lock(&state)?;
    let task = tasks::get_task(&conn, &id).map_err(db_err)?.ok_or("Task not found")?;
    let notes = tasks::list_notes(&conn, &id).map_err(db_err)?;
    let children: Vec<Task> = tasks::list_tasks(&conn, true)
        .map_err(db_err)?
        .into_iter()
        .filter(|t| t.parent_id.as_deref() == Some(id.as_str()))
        .collect();
    let mut sources = Vec::new();
    for c in captures::list_changes_for_task(&conn, &id).map_err(db_err)? {
        let cap = captures::get_capture(&conn, &c.capture_id).map_err(db_err)?;
        let inputs = captures::list_inputs(&conn, &c.capture_id).map_err(db_err)?;
        sources.push(SourceRef {
            capture_id: c.capture_id.clone(),
            captured_at: cap.map(|c| c.created_at).unwrap_or_default(),
            filenames: inputs
                .iter()
                .map(|i| i.filename.clone().unwrap_or_else(|| "Pasted text".to_string()))
                .collect(),
            kind: c.kind,
            reason: c.reason,
            evidence: c.evidence,
            undone: c.undone,
        });
    }
    Ok(TaskDetail { task, notes, children, sources })
}

#[tauri::command]
pub fn create_task(state: State<'_, AppState>, new: NewTask) -> CmdResult<Task> {
    if new.title.trim().is_empty() {
        return Err("A task needs a title.".to_string());
    }
    tasks::insert_task(&*lock(&state)?, &new).map_err(db_err)
}

#[tauri::command]
pub fn update_task(state: State<'_, AppState>, id: String, patch: TaskPatch) -> CmdResult<Task> {
    tasks::update_task(&*lock(&state)?, &id, &patch).map_err(db_err)
}

#[tauri::command]
pub fn add_note(state: State<'_, AppState>, task_id: String, body: String) -> CmdResult<Note> {
    if body.trim().is_empty() {
        return Err("The note is empty.".to_string());
    }
    tasks::add_note(&*lock(&state)?, &task_id, body.trim(), None).map_err(db_err)
}

#[tauri::command]
pub fn delete_note(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    tasks::delete_note(&*lock(&state)?, &id).map_err(db_err)
}

#[tauri::command]
pub fn list_categories(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    tasks::list_categories(&*lock(&state)?).map_err(db_err)
}

#[tauri::command]
pub fn up_next(state: State<'_, AppState>, limit: usize) -> CmdResult<Vec<Ranked>> {
    let all = tasks::list_tasks(&*lock(&state)?, true).map_err(db_err)?;
    let today = chrono::Local::now().date_naive();
    Ok(rank_up_next(&all, today, limit))
}

fn ai_config(settings: &Settings) -> CmdResult<AiConfig> {
    let api_key = secrets::get_api_key()?.ok_or_else(|| AiError::NoKey.to_string())?;
    Ok(AiConfig {
        api_key,
        model: settings.model.clone(),
        effort: settings.effort.clone(),
        base_url: DEFAULT_BASE_URL.to_string(),
    })
}

#[tauri::command]
pub async fn start_capture(
    state: State<'_, AppState>,
    text: Option<String>,
    paths: Vec<String>,
) -> CmdResult<Capture> {
    let text = text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    if text.is_none() && paths.is_empty() {
        return Err("Paste some text or drop a file first.".to_string());
    }

    let mut inputs: Vec<Extracted> = Vec::new();
    if let Some(t) = &text {
        inputs.push(Extracted::Text { filename: "Pasted text".to_string(), text: t.clone() });
    }
    for p in &paths {
        inputs.push(extract_file(Path::new(p)).map_err(|e| e.to_string())?);
    }
    if total_chars(&inputs) > MAX_TOTAL_CHARS {
        return Err("That is too much text for one pass. Split this into two captures.".to_string());
    }

    let (settings, cfg, capture_id, request) = {
        let conn = lock(&state)?;
        let settings = settings::load(&conn);
        let provider = Provider::parse(&settings.provider).unwrap_or(Provider::ApiKey);
        let cfg = match provider {
            Provider::ApiKey => Some(ai_config(&settings)?),
            Provider::ClaudeCode => None,
        };
        let capture = captures::insert_capture(&conn, &settings.model).map_err(db_err)?;
        for input in &inputs {
            match input {
                Extracted::Text { filename, text } => {
                    let kind = if filename == "Pasted text" { InputKind::Paste } else { InputKind::File };
                    let name = if kind == InputKind::Paste { None } else { Some(filename.as_str()) };
                    captures::insert_input(&conn, &capture.id, kind, name, text).map_err(db_err)?;
                }
                Extracted::Pdf { filename, .. } => {
                    captures::insert_input(&conn, &capture.id, InputKind::File, Some(filename), "(PDF)").map_err(db_err)?;
                }
            }
        }
        let open_tasks = tasks::list_tasks(&conn, false).map_err(db_err)?;
        let categories = tasks::list_categories(&conn).map_err(db_err)?;
        let request = ExtractionRequest {
            today: chrono::Local::now().format("%Y-%m-%d").to_string(),
            user_name: settings.user_name.clone(),
            manager_name: settings.manager_name.clone(),
            open_tasks,
            categories,
            inputs,
        };
        (settings, cfg, capture.id, request)
    };

    let result: Result<_, String> = match cfg {
        Some(cfg) => run_extraction(&cfg, &request).await.map_err(|e| e.to_string()),
        None => match cc_run(&settings, &state.data_dir) {
            Ok(run) => claude_code::run_extraction(&run, &request).await.map_err(|e| e.to_string()),
            Err(e) => Err(e),
        },
    };
    let conn = lock(&state)?;
    match result {
        Ok((resp, usage)) => {
            let open: HashMap<String, Option<String>> =
                request.open_tasks.iter().map(|t| (t.id.clone(), t.parent_id.clone())).collect();
            let proposal = resolve(resp, &open);
            captures::set_proposal(&conn, &capture_id, &proposal, (usage.input_tokens, usage.output_tokens))
                .map_err(db_err)?;
            captures::get_capture(&conn, &capture_id).map_err(db_err)?.ok_or_else(|| "Capture vanished".to_string())
        }
        Err(e) => {
            let _ = captures::set_status(&conn, &capture_id, CaptureStatus::Failed);
            Err(e)
        }
    }
}

#[tauri::command]
pub fn apply_capture(state: State<'_, AppState>, capture_id: String, selected: Vec<usize>) -> CmdResult<Vec<Change>> {
    let conn = lock(&state)?;
    let capture = captures::get_capture(&conn, &capture_id).map_err(db_err)?.ok_or("Capture not found")?;
    let proposal = capture.proposal.ok_or("This capture has nothing to apply.")?;
    apply_proposal(&conn, &capture_id, &proposal, &selected).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn discard_capture(state: State<'_, AppState>, capture_id: String) -> CmdResult<()> {
    captures::set_status(&*lock(&state)?, &capture_id, CaptureStatus::Discarded).map_err(db_err)
}

#[tauri::command]
pub fn list_captures(state: State<'_, AppState>, limit: usize) -> CmdResult<Vec<CaptureSummary>> {
    let conn = lock(&state)?;
    let mut out = Vec::new();
    for capture in captures::list_captures(&conn, limit).map_err(db_err)? {
        let inputs = captures::list_inputs(&conn, &capture.id).map_err(db_err)?;
        let changes = captures::list_changes_for_capture(&conn, &capture.id).map_err(db_err)?;
        out.push(CaptureSummary { capture, inputs, changes });
    }
    Ok(out)
}

#[tauri::command]
pub fn undo_change(state: State<'_, AppState>, change_id: String, force: bool) -> CmdResult<UndoOutcome> {
    apply_undo(&*lock(&state)?, &change_id, force).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> CmdResult<SettingsView> {
    let conn = lock(&state)?;
    let settings = settings::load(&conn);
    let key = secrets::get_api_key().unwrap_or(None);
    let provider_status = state.status.lock().map(|s| s.clone()).unwrap_or_default();
    Ok(SettingsView {
        settings,
        has_key: key.is_some(),
        key_hint: key.as_deref().map(secrets::masked),
        data_dir: state.data_dir.to_string_lossy().to_string(),
        provider_status,
    })
}

/// Re-checks the selected provider, persists the result and the auto-chosen provider.
#[tauri::command]
pub async fn probe_provider(state: State<'_, AppState>) -> CmdResult<ProviderStatus> {
    let settings = settings::load(&*lock(&state)?);
    let has_key = secrets::get_api_key().unwrap_or(None).is_some();
    let status = compute_status(&settings, has_key).await;
    let conn = lock(&state)?;
    let current = settings::load(&conn);
    if current.provider != settings.provider {
        // The user changed the provider while this probe ran; its result is for the old choice.
        return Ok(state.status.lock().map(|s| s.clone()).unwrap_or_default());
    }
    if current.provider != status.provider {
        settings::set_provider(&conn, &status.provider).map_err(db_err)?;
    }
    settings::save_status(&conn, &status).map_err(db_err)?;
    if let Ok(mut cached) = state.status.lock() {
        *cached = status.clone();
    }
    Ok(status)
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> CmdResult<()> {
    settings::save(&*lock(&state)?, &settings).map_err(db_err)
}

#[tauri::command]
pub fn set_api_key(key: String) -> CmdResult<()> {
    secrets::set_api_key(&key)
}

#[tauri::command]
pub fn clear_api_key() -> CmdResult<()> {
    secrets::clear_api_key()
}

#[tauri::command]
pub async fn test_connection(state: State<'_, AppState>) -> CmdResult<String> {
    let settings = settings::load(&*lock(&state)?);
    match Provider::parse(&settings.provider) {
        Some(Provider::ClaudeCode) => {
            let run = cc_run(&settings, &state.data_dir)?;
            claude_code::test_connection(&run).await.map_err(|e| e.to_string())
        }
        _ => {
            let cfg = ai_config(&settings)?;
            ai_test_connection(&cfg).await.map_err(|e| e.to_string())
        }
    }
}

#[tauri::command]
pub fn open_data_folder(app: tauri::AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(state.data_dir.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_parse_accepts_two_values_only() {
        assert_eq!(Provider::parse("claude_code"), Some(Provider::ClaudeCode));
        assert_eq!(Provider::parse("api_key"), Some(Provider::ApiKey));
        assert_eq!(Provider::parse(""), None);
        assert_eq!(Provider::parse("openai"), None);
    }

    #[test]
    fn unset_provider_defaults_by_readiness() {
        assert_eq!(choose_provider("", true), "claude_code");
        assert_eq!(choose_provider("", false), "api_key");
        assert_eq!(choose_provider("api_key", true), "api_key");
        assert_eq!(choose_provider("claude_code", false), "claude_code");
    }
}
