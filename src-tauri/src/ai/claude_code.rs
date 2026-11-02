//! Claude Code provider: runs the `claude` CLI headlessly so the user's Claude
//! subscription does the extraction. Nothing here reads or stores tokens; the
//! only credential path is whatever `claude` itself is signed in with.

use crate::ai::client::AiUsage;
use crate::ai::ops::ModelResponse;
use crate::ai::prompt::{user_text, ExtractionRequest, SYSTEM_PROMPT};
use crate::ai::schema::operations_schema;
use crate::ingest::Extracted;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

const AUTH_TIMEOUT: Duration = Duration::from_secs(20);
const ENV_REMOVED: [&str; 3] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL"];
const EXE_NAMES: [&str; 3] = ["claude.exe", "claude.cmd", "claude"];

#[derive(Debug, thiserror::Error)]
pub enum CcError {
    #[error("Claude Code is not installed, or Unpack cannot find it. Set the path in Settings.")]
    NotFound,
    #[error("Claude Code is signed out. Run `claude` in a terminal and sign in.")]
    SignedOut,
    #[error(
        "Claude Code is signed in with an API key, not your Claude account, so it would bill per token. Run `claude` and sign in with your Claude account, or pick the API key provider here."
    )]
    ApiKeyLogin,
    #[error("Your Claude seat is rate-limited right now. Try again in a few minutes or switch to the API key in Settings.")]
    RateLimited,
    #[error("{0}")]
    Failed(String),
    #[error("Claude Code returned no structured result.")]
    NoStructuredOutput,
    #[error("Claude Code took too long.")]
    Timeout,
    #[error("Claude Code's answer could not be read: {0}")]
    BadJson(String),
    #[error("{0}")]
    Io(String),
}

/// Output of `claude auth status`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AuthStatus {
    pub logged_in: bool,
    pub auth_method: String,
    pub email: String,
    pub org_name: String,
    pub subscription_type: String,
}

pub fn parse_auth_status(json: &str) -> Result<AuthStatus, CcError> {
    serde_json::from_str(json.trim()).map_err(|e| CcError::BadJson(e.to_string()))
}

/// Ready only when signed in with a Claude account (not an API key).
pub fn readiness(status: &AuthStatus) -> Result<String, CcError> {
    if !status.logged_in {
        return Err(CcError::SignedOut);
    }
    if status.auth_method != "claude.ai" {
        return Err(CcError::ApiKeyLogin);
    }
    Ok(format!("Signed in as {} ({}, {} plan)", status.email, status.org_name, status.subscription_type))
}

/// The parts of the `--output-format json` result envelope the app uses.
#[derive(Debug, Clone)]
pub struct Envelope {
    pub structured_output: Option<Value>,
    pub usage: AiUsage,
    pub model: String,
}

pub fn parse_envelope(stdout: &str) -> Result<Envelope, CcError> {
    let result = stdout
        .lines()
        .rev()
        .filter_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
        .find(|v| v["type"] == "result")
        .ok_or_else(|| CcError::BadJson("no result line in the output".to_string()))?;
    if result["is_error"].as_bool().unwrap_or(false) {
        let text = result["result"].as_str().unwrap_or("");
        if text.trim_start().to_ascii_lowercase().starts_with("not logged in") {
            return Err(CcError::SignedOut);
        }
        return Err(match result["api_error_status"].as_u64() {
            Some(401) => CcError::SignedOut,
            Some(429) => CcError::RateLimited,
            _ => CcError::Failed(
                result["result"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or("Claude Code reported an error")
                    .to_string(),
            ),
        });
    }
    let u = &result["usage"];
    let sum = |k: &str| u[k].as_i64().unwrap_or(0);
    Ok(Envelope {
        structured_output: result.get("structured_output").filter(|v| v.is_object()).cloned(),
        usage: AiUsage {
            input_tokens: sum("input_tokens") + sum("cache_read_input_tokens") + sum("cache_creation_input_tokens"),
            output_tokens: sum("output_tokens"),
        },
        model: result["modelUsage"].as_object().and_then(|m| m.keys().next().cloned()).unwrap_or_default(),
    })
}

/// Finds the Claude Code executable: an explicit override, then `PATH`, then
/// the usual install folders.
pub fn locate(override_path: &str) -> Option<PathBuf> {
    let o = override_path.trim();
    if !o.is_empty() {
        let p = PathBuf::from(o);
        return if p.is_file() { Some(p) } else { None };
    }
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|v| std::env::split_paths(&v).collect()).unwrap_or_default();
    if let Some(home) = std::env::var_os("USERPROFILE") {
        dirs.push(PathBuf::from(home).join(".local").join("bin"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Programs").join("claude"));
    }
    for d in dirs {
        for name in EXE_NAMES {
            let p = d.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// A `claude` command with the subscription as its only credential and no console window.
/// `.cmd`/`.bat` shims are handled by the standard library, which keeps every argument intact.
pub fn command(exe: &Path) -> Command {
    let mut cmd = Command::new(exe);
    for var in ENV_REMOVED {
        cmd.env_remove(var);
    }
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    cmd.kill_on_drop(true);
    cmd
}

pub async fn auth_status(exe: &Path) -> Result<AuthStatus, CcError> {
    let mut cmd = command(exe);
    cmd.args(["auth", "status"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let out = tokio::time::timeout(AUTH_TIMEOUT, cmd.output())
        .await
        .map_err(|_| CcError::Timeout)?
        .map_err(|e| CcError::Io(e.to_string()))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    match parse_auth_status(&stdout) {
        Ok(s) => Ok(s),
        Err(_) if !out.status.success() => Err(CcError::SignedOut),
        Err(e) => Err(e),
    }
}

#[derive(Debug, Clone)]
pub struct CcRun {
    pub exe: PathBuf,
    pub model: String,
    pub effort: String,
    /// Per-capture PDF copies live under here; the folder is created and removed per run.
    pub scratch_dir: PathBuf,
    pub timeout: Duration,
}

async fn run_print(run: &CcRun, prompt: &str, schema: &Value, extra: &[String], cwd: &Path) -> Result<Envelope, CcError> {
    let mut cmd = command(&run.exe);
    cmd.arg("-p")
        .args(["--output-format", "json"])
        .arg("--json-schema")
        .arg(schema.to_string())
        .arg("--system-prompt")
        .arg(SYSTEM_PROMPT)
        .args(["--model", &run.model, "--effort", &run.effort])
        .args(["--no-session-persistence", "--disable-slash-commands", "--permission-prompts", "none", "--strict-mcp-config"])
        // No user/project settings: no hooks, plugins or CLAUDE.md context leak into the extraction.
        .args(["--setting-sources", ""])
        .args(extra)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => CcError::NotFound,
        _ => CcError::Io(e.to_string()),
    })?;
    let mut stdin = child.stdin.take().ok_or_else(|| CcError::Io("no stdin".to_string()))?;
    let text = prompt.to_string();
    let writer = tokio::spawn(async move {
        let _ = stdin.write_all(text.as_bytes()).await;
        let _ = stdin.shutdown().await;
    });
    let out = match tokio::time::timeout(run.timeout, child.wait_with_output()).await {
        Ok(r) => r.map_err(|e| CcError::Io(e.to_string()))?,
        Err(_) => return Err(CcError::Timeout),
    };
    let _ = writer.await;
    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.trim().is_empty() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(CcError::Failed(if err.is_empty() {
            format!("Claude Code exited with code {}", out.status.code().unwrap_or(-1))
        } else {
            err
        }));
    }
    parse_envelope(&stdout)
}

/// Copies the PDFs of a request into a fresh folder so only that folder is exposed to the Read tool.
/// (temp folder, staged (filename, path) pairs)
type Staged = (Option<PathBuf>, Vec<(String, String)>);

fn stage_pdfs(run: &CcRun, req: &ExtractionRequest) -> Result<Staged, CcError> {
    let pdfs: Vec<(&str, &str)> = req
        .inputs
        .iter()
        .filter_map(|i| match i {
            Extracted::Pdf { filename, path, .. } => Some((filename.as_str(), path.as_str())),
            _ => None,
        })
        .collect();
    if pdfs.is_empty() {
        return Ok((None, vec![]));
    }
    let dir = run.scratch_dir.join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).map_err(|e| CcError::Io(e.to_string()))?;
    let mut staged = Vec::new();
    for (name, src) in pdfs {
        let dest = dir.join(name);
        std::fs::copy(src, &dest).map_err(|e| CcError::Io(format!("{name}: {e}")))?;
        staged.push((name.to_string(), dest.to_string_lossy().to_string()));
    }
    Ok((Some(dir), staged))
}

pub async fn run_extraction(run: &CcRun, req: &ExtractionRequest) -> Result<(ModelResponse, AiUsage), CcError> {
    std::fs::create_dir_all(&run.scratch_dir).map_err(|e| CcError::Io(e.to_string()))?;
    let (pdf_dir, staged) = stage_pdfs(run, req)?;
    let extra: Vec<String> = match &pdf_dir {
        Some(d) => vec!["--tools".into(), "Read".into(), "--add-dir".into(), d.to_string_lossy().to_string()],
        None => vec!["--tools".into(), String::new()],
    };
    let prompt = user_text(req, &staged);
    let result = run_print(run, &prompt, &operations_schema(), &extra, &run.scratch_dir).await;
    if let Some(d) = pdf_dir {
        let _ = std::fs::remove_dir_all(d);
    }
    let env = result?;
    let out = env.structured_output.ok_or(CcError::NoStructuredOutput)?;
    let parsed: ModelResponse = serde_json::from_value(out).map_err(|e| CcError::BadJson(e.to_string()))?;
    Ok((parsed, env.usage))
}

/// Sends a tiny request; returns the model name Claude Code reports.
pub async fn test_connection(run: &CcRun) -> Result<String, CcError> {
    std::fs::create_dir_all(&run.scratch_dir).map_err(|e| CcError::Io(e.to_string()))?;
    let schema = json!({"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false});
    let env = run_print(run, "Reply with OK.", &schema, &["--tools".to_string(), String::new()], &run.scratch_dir).await?;
    Ok(if env.model.is_empty() { run.model.clone() } else { env.model })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const AUTH_OK: &str = r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty","email":"sam@example.com","orgName":"Example Org","subscriptionType":"team"}"#;

    fn envelope(extra: &str) -> String {
        format!(
            r#"{{"type":"result","subtype":"success","is_error":false,"usage":{{"input_tokens":2,"cache_creation_input_tokens":70060,"cache_read_input_tokens":0,"output_tokens":150}},"modelUsage":{{"claude-haiku-5-5":{{"inputTokens":2}}}},"api_error_status":null,"result":"{{}}"{extra}}}"#
        )
    }

    #[test]
    fn auth_status_parses_claude_ai_login() {
        let st = parse_auth_status(AUTH_OK).unwrap();
        assert!(st.logged_in);
        assert_eq!(readiness(&st).unwrap(), "Signed in as sam@example.com (Example Org, team plan)");
    }

    #[test]
    fn auth_status_with_api_key_login_is_not_ready() {
        let st = parse_auth_status(&AUTH_OK.replace("claude.ai", "apiKey")).unwrap();
        assert!(matches!(readiness(&st), Err(CcError::ApiKeyLogin)));
    }

    #[test]
    fn auth_status_logged_out() {
        let st = parse_auth_status(r#"{"loggedIn":false}"#).unwrap();
        assert!(matches!(readiness(&st), Err(CcError::SignedOut)));
        assert!(matches!(parse_auth_status("not json"), Err(CcError::BadJson(_))));
    }

    #[test]
    fn envelope_success_yields_structured_output_and_usage() {
        let out = envelope(r#","structured_output":{"summary":"s","operations":[]}"#);
        let env = parse_envelope(&out).unwrap();
        assert_eq!(env.structured_output.unwrap()["summary"], "s");
        assert_eq!(env.usage.input_tokens, 70062);
        assert_eq!(env.usage.output_tokens, 150);
        assert_eq!(env.model, "claude-haiku-5-5");
    }

    #[test]
    fn envelope_error_401_maps_to_signed_out() {
        let out = envelope("").replace(r#""is_error":false"#, r#""is_error":true"#).replace(r#""api_error_status":null"#, r#""api_error_status":401"#);
        assert!(matches!(parse_envelope(&out), Err(CcError::SignedOut)));
    }

    #[test]
    fn envelope_error_429_maps_to_rate_limited() {
        let out = envelope("").replace(r#""is_error":false"#, r#""is_error":true"#).replace(r#""api_error_status":null"#, r#""api_error_status":429"#);
        assert!(matches!(parse_envelope(&out), Err(CcError::RateLimited)));
    }

    #[test]
    fn envelope_other_error_carries_result_text() {
        let out = envelope("").replace(r#""is_error":false"#, r#""is_error":true"#).replace(r#""result":"{}""#, r#""result":"Something broke""#);
        match parse_envelope(&out) {
            Err(CcError::Failed(m)) => assert_eq!(m, "Something broke"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn envelope_without_structured_output_is_error() {
        assert!(matches!(parse_envelope(&envelope("")), Ok(Envelope { structured_output: None, .. })));
        assert!(matches!(parse_envelope("garbage"), Err(CcError::BadJson(_))));
        assert!(matches!(parse_envelope(r#"{"type":"system"}"#), Err(CcError::BadJson(_))));
    }

    #[test]
    fn command_runs_the_executable_directly() {
        let c = command(Path::new(r"C:\tools\claude.cmd"));
        assert!(c.as_std().get_program().to_string_lossy().ends_with("claude.cmd"));
        let c = command(Path::new(r"C:\tools\claude.exe"));
        assert!(c.as_std().get_program().to_string_lossy().ends_with("claude.exe"));
        for v in ENV_REMOVED {
            assert!(c.as_std().get_envs().any(|(k, val)| k == v && val.is_none()), "{v} must be removed");
        }
    }

    #[test]
    fn signed_out_result_text_maps_to_signed_out() {
        let out = r#"{"type":"result","is_error":true,"api_error_status":null,"result":"Not logged in · Please run /login"}"#;
        assert!(matches!(parse_envelope(out), Err(CcError::SignedOut)));
    }

    #[test]
    fn locate_prefers_override() {
        let dir = std::env::temp_dir().join("unpack_locate_test");
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("claude.cmd");
        std::fs::write(&fake, "@echo off\r\n").unwrap();
        assert_eq!(locate(fake.to_str().unwrap()), Some(fake.clone()));
        assert_eq!(locate(dir.join("missing.exe").to_str().unwrap()), None, "a wrong override never falls back to another claude");
    }

    #[test]
    fn error_texts_are_plain_sentences() {
        assert_eq!(CcError::NotFound.to_string(), "Claude Code is not installed, or Unpack cannot find it. Set the path in Settings.");
        assert_eq!(CcError::SignedOut.to_string(), "Claude Code is signed out. Run `claude` in a terminal and sign in.");
        assert_eq!(CcError::RateLimited.to_string(), "Your Claude seat is rate-limited right now. Try again in a few minutes or switch to the API key in Settings.");
        assert_eq!(CcError::NoStructuredOutput.to_string(), "Claude Code returned no structured result.");
        assert_eq!(CcError::Timeout.to_string(), "Claude Code took too long.");
    }

    // ---- spawn tests against a fake CLI (Windows .cmd shim) ----

    const GOOD_ENVELOPE: &str = r#"{"type":"result","subtype":"success","is_error":false,"usage":{"input_tokens":10,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":20},"modelUsage":{"claude-haiku-5-5":{}},"api_error_status":null,"result":"","structured_output":{"summary":"one task","operations":[{"op":"create","temp_id":"t1","task_id":null,"title":"Write the report","description":null,"priority":"high","category":null,"due_date":null,"parent_id":null,"body":null,"reason":"asked","evidence":null}]}}"#;

    /// Writes a fake `claude.cmd` that prints `envelope` (after optional `prelude` lines) and returns its path.
    fn fake_claude(name: &str, envelope: &str, prelude: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("unpack_fake_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("envelope.json"), envelope).unwrap();
        let script = format!("@echo off\r\n{prelude}\r\ntype \"{}\"\r\n", dir.join("envelope.json").display());
        let exe = dir.join("claude.cmd");
        std::fs::write(&exe, script).unwrap();
        (exe, dir)
    }

    fn run_for(exe: PathBuf, dir: &Path, timeout_secs: u64) -> CcRun {
        CcRun {
            exe,
            model: "claude-haiku-5-5".into(),
            effort: "low".into(),
            scratch_dir: dir.join("scratch"),
            timeout: Duration::from_secs(timeout_secs),
        }
    }

    fn request(inputs: Vec<Extracted>) -> ExtractionRequest {
        ExtractionRequest {
            today: "2026-11-01".into(),
            user_name: "Sam".into(),
            manager_name: String::new(),
            open_tasks: vec![],
            categories: vec![],
            inputs,
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn fake_cli_run_returns_model_response() {
        let (exe, dir) = fake_claude("ok", GOOD_ENVELOPE, "");
        let run = run_for(exe, &dir, 30);
        let req = request(vec![Extracted::Text { filename: "a.txt".into(), text: "hi".into() }]);
        let (resp, usage) = run_extraction(&run, &req).await.unwrap();
        assert_eq!(resp.operations.len(), 1);
        assert_eq!(resp.operations[0].title.as_deref(), Some("Write the report"));
        assert_eq!(usage.output_tokens, 20);
        assert_eq!(test_connection(&run).await.unwrap(), "claude-haiku-5-5");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn signed_out_envelope_maps_to_signed_out() {
        let env = GOOD_ENVELOPE.replace(r#""is_error":false"#, r#""is_error":true"#).replace(r#""api_error_status":null"#, r#""api_error_status":401"#);
        let (exe, dir) = fake_claude("401", &env, "");
        let run = run_for(exe, &dir, 30);
        let err = run_extraction(&run, &request(vec![])).await.unwrap_err();
        assert!(matches!(err, CcError::SignedOut), "{err}");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn large_prompt_goes_through_stdin() {
        let (exe, dir) = fake_claude("stdin", GOOD_ENVELOPE, "");
        let capture = dir.join("stdin.txt");
        let script = format!("@echo off\r\nmore > \"{}\"\r\ntype \"{}\"\r\n", capture.display(), dir.join("envelope.json").display());
        std::fs::write(&exe, script).unwrap();
        let run = run_for(exe, &dir, 60);
        let big = "x".repeat(200_000);
        let req = request(vec![Extracted::Text { filename: "big.txt".into(), text: big }]);
        run_extraction(&run, &req).await.unwrap();
        let got = std::fs::metadata(&capture).unwrap().len();
        assert!(got >= 200_000, "stdin received {got} bytes");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn pdf_temp_folder_is_removed_after_run() {
        let (exe, dir) = fake_claude("pdf", GOOD_ENVELOPE, "");
        let pdf = dir.join("deck.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        let run = run_for(exe, &dir, 30);
        let req = request(vec![Extracted::Pdf { filename: "deck.pdf".into(), base64: "JVBERg==".into(), path: pdf.to_string_lossy().to_string() }]);
        run_extraction(&run, &req).await.unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(&run.scratch_dir).unwrap().flatten().collect();
        assert!(leftovers.is_empty(), "scratch dir should be empty, has {}", leftovers.len());
        assert!(pdf.is_file(), "the user's original file is untouched");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn timeout_kills_the_child() {
        let (exe, dir) = fake_claude("slow", GOOD_ENVELOPE, "ping -n 10 127.0.0.1 >nul");
        let run = run_for(exe, &dir, 1);
        let started = std::time::Instant::now();
        let err = run_extraction(&run, &request(vec![])).await.unwrap_err();
        assert!(matches!(err, CcError::Timeout), "{err}");
        assert!(started.elapsed() < Duration::from_secs(4), "took {:?}", started.elapsed());
    }

    /// Runs only when the real CLI is installed and signed in with a Claude account.
    #[tokio::test]
    async fn real_cli_test_connection_when_signed_in() {
        let Some(exe) = locate("") else {
            eprintln!("skipped: claude not installed");
            return;
        };
        let Ok(st) = auth_status(&exe).await else {
            eprintln!("skipped: auth status unavailable");
            return;
        };
        if readiness(&st).is_err() {
            eprintln!("skipped: not signed in with a Claude account");
            return;
        }
        let dir = std::env::temp_dir().join("unpack_real_cli");
        let run = run_for(exe, &dir, 120);
        let model = test_connection(&run).await.unwrap();
        assert!(!model.is_empty());
    }
}
