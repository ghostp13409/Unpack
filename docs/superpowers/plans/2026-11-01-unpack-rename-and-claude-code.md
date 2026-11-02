# Unpack rename, Claude Code provider, title bar, headless mode — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename the shipped v0.1.0 app to Unpack, add a Claude Code provider that uses the user's Claude subscription next to the existing API-key provider, replace the native title bar with the app's own, add a headless import mode, scrub company context from the repo, then build, install, publish to GitHub and load the user's real tasks.

**Architecture:** The Rust backend gains `ai/claude_code.rs` (spawn `claude -p`, parse its JSON envelope) and `headless.rs` (CLI modes run before the Tauri window); `commands.rs` dispatches on a `provider` setting and caches a `ProviderStatus`. The frontend gains a `TitleBar` component and a provider section in Settings. Everything downstream of `ModelResponse` (resolve, apply, undo, views) is untouched.

**Tech Stack:** as v0.1.0 (Tauri 2.12, Rust 2024, rusqlite, reqwest, React 19, zustand, vitest) plus `tokio::process` (already a dependency via tokio) and `@tauri-apps/api/window`.

**Spec:** `docs/superpowers/specs/2026-11-01-unpack-rename-and-claude-code-design.md` (read its addendum too).

## Global Constraints

- Never pass `--bare` to `claude`; the user prompt goes to `claude` on stdin, never on argv; spawned `claude` processes get `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_BASE_URL` removed from their environment and run with `CREATE_NO_WINDOW` (`0x08000000`) on Windows.
- "Ready" for Claude Code means `auth status` exit 0, `loggedIn: true` and `authMethod == "claude.ai"`.
- Error texts are plain sentences, exactly as the spec words them.
- The API-key request body (`build_body`) is byte-for-byte unchanged; existing httpmock tests must keep passing untouched.
- The 350,000-char guard, per-file size caps, snake_case enums, keyring-only key storage, plain CSS tokens all stay.
- No company context in the repo: no real names of people, customers, products, projects, orgs, emails or workspace paths in code, fixtures, mock data, or docs. A denylist kept **outside** the repo (session scratchpad) is the check; the plan does not list the terms.
- The old name may remain only in `README.md` (one "formerly" line) and in the Unpack design/plan docs where they describe the rename; `grep -ri pgtasktracker` over everything else (excluding `.git`, lockfiles are regenerated) returns nothing.
- Commit after every task with a conventional-commit message; identity for new commits: `git config user.email gajjarparth09@gmail.com` set locally in Task 1.

## Review Focus

1. Provider `claude_code`, Claude Code signed out: a capture fails with the sign-out sentence, the capture row is `failed`, the process never hangs (Task 4 fake CLI test `signed_out_envelope_maps_to_signed_out`, Task 5 `start_capture` path).
2. A 200 KB transcript reaches `claude` intact through stdin (Task 4 `large_prompt_goes_through_stdin`).
3. The API-key body is unchanged (Task 5 keeps `prompt.rs` body tests and `client.rs` httpmock tests untouched; Task 5 adds `api_body_unchanged_by_user_text`).
4. Both providers feed `resolve` through the same `ModelResponse` type (Task 5 `start_capture` has one `resolve` call; Task 8 `--import` reuses it).
5. A grandchild or orphan PDF temp folder is never left behind: the per-capture folder is removed on success and failure (Task 4 `pdf_temp_folder_is_removed_after_run`).

---

### Task 1: Rename to Unpack

**Files:**
- Modify: `package.json`, `package-lock.json` (regenerate), `index.html`, `README.md`, `src/lib/mockApi.ts:252`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` (regenerate), `src-tauri/src/main.rs`, `src-tauri/src/secrets.rs:4`, `src-tauri/tauri.conf.json`, `docs/overview.md`, `docs/superpowers/HANDOFF.md`
- Rename: `docs/superpowers/specs/2026-11-01-pgtasktracker-design.md` → `docs/superpowers/specs/2026-11-01-unpack-v1-design.md`; `docs/superpowers/plans/2026-11-01-pgtasktracker.md` → `docs/superpowers/plans/2026-11-01-unpack-v1.md` (the spec's suggested names collide with this plan's; `-v1` is the ruling)
- Modify: the Unpack design doc's references to the two renamed files.

**Interfaces:**
- Produces: crate `unpack`, lib `unpack_lib`, npm package `unpack`, product name `Unpack`, identifier `com.parth.unpack`, keyring service `Unpack`, window title `Unpack`.

- [ ] **Step 1: Write the check script in the scratchpad** (not in the repo): `rename-check.sh` runs `grep -rIi pgtasktracker --exclude-dir=.git --exclude-dir=node_modules --exclude-dir=target --exclude-dir=dist --exclude-dir=gen . | grep -v "^./README.md" | grep -v "unpack-rename-and-claude-code"` and exits 1 if anything prints.
- [ ] **Step 2: Run it** → Expected: exit 1, listing `index.html`, `package.json`, `Cargo.toml`, `main.rs`, `secrets.rs`, `tauri.conf.json`, `mockApi.ts`, docs.
- [ ] **Step 3: Apply the rename**: values from Interfaces; `git mv` the two docs; `docs/overview.md` heading `# Unpack`; README first line `# Unpack` plus one line "Formerly PGTaskTracker (v0.1.0)."; `mockApi.ts` data dir string `com.parth.unpack`; HANDOFF prose. Then `npm install` and `cd src-tauri && cargo check` to regenerate both lockfiles. `git config user.email gajjarparth09@gmail.com`.
- [ ] **Step 4: Run the check script** → Expected: exit 0, no output.
- [ ] **Step 5: Run `cd src-tauri && cargo test --lib` and `npm run build`** → Expected: 79 passed; build passes.
- [ ] **Step 6: Commit** `git add -A && git commit -m "refactor: rename PGTaskTracker to Unpack"`

---

### Task 2: Scrub company context from the repo

**Files:**
- Modify: `src/lib/mockApi.ts` (seed tasks, notes, captures, settings, canned proposal), `src/store/derived.test.ts`, `src-tauri/src/settings.rs` (default `user_name` → `""`), `src-tauri/src/ai/{client,ops,prompt}.rs` test fixtures, `src-tauri/src/ingest/{eml,vtt}.rs` fixtures, `src-tauri/src/repo/tasks.rs` fixtures, `docs/superpowers/specs/2026-11-01-unpack-v1-design.md`, `docs/superpowers/plans/2026-11-01-unpack-v1.md`, `docs/superpowers/specs/2026-11-01-unpack-rename-and-claude-code-design.md`, `docs/superpowers/HANDOFF.md`, `docs/overview.md`
- Keep: `Cargo.toml` `authors` (the user's own name is not company context).

**Interfaces:**
- Produces: fictional fixture vocabulary used by every later task: people "Sam" (user), "Alex Morgan" (manager), "Jordan"; categories "Platform", "Observability", "Pricing", "Cloud", "Admin"; projects "Atlas", "Beacon"; emails `@example.com`; workspace path written as `<repo>`.

- [ ] **Step 1: Write `scrub-check.sh` in the scratchpad** that runs `grep -rnwF -f <scratchpad>/denylist.txt` over the repo excluding `.git`, `node_modules`, `target`, `dist`, `gen`, lockfiles and image files, and exits 1 on any hit.
- [ ] **Step 2: Run it** → Expected: exit 1 with the hits listed (about 70 lines).
- [ ] **Step 3: Replace** every hit with the fictional vocabulary. Rules: keep test semantics (same lengths where a test asserts a clip, same wrapped-line shape in the `.vtt` fixture, same header order in the `.eml` fixture); `settings.rs` default `user_name` becomes `""` and its test asserts `""`; the Settings hint under "Your name" already explains why it matters. In the Unpack design doc replace the user's email, org names, account label and `<workspace>` with "the user's work Claude account" and `<workspace>`.
- [ ] **Step 4: Run the scrub check** → Expected: exit 0.
- [ ] **Step 5: Run `cargo test --lib`, `npm test`, `npm run build`** → Expected: 79 passed; 22 passed; build passes.
- [ ] **Step 6: Commit** `git commit -am "chore: fictional fixtures and mock data; no company context in the repo"`

---

### Task 3: Provider settings and status cache

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Test: same file (`#[cfg(test)]`)

**Interfaces:**
- Produces: `Settings { model, effort, user_name, manager_name, theme, provider: String, claude_code_path: String }` with defaults `provider: ""` (unset) and `claude_code_path: ""`; `KEYS` grows to 7.
- Produces: `#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)] pub struct ProviderStatus { pub provider: String, pub ready: bool, pub detail: String, pub account: String, pub checked_at: String }` and `pub fn load_status(conn: &Connection) -> ProviderStatus`, `pub fn save_status(conn: &Connection, s: &ProviderStatus) -> rusqlite::Result<()>` stored under keys `provider_status.*` in the `settings` table.

- [ ] **Step 1: Write failing tests** `provider_fields_default_to_unset` (load → `provider == ""`, `claude_code_path == ""`), `provider_fields_roundtrip` (save `provider: "claude_code"`, `claude_code_path: r"C:\x\claude.exe"` → load equal), `status_roundtrip_and_default` (default `ready == false`; save then load equal).
- [ ] **Step 2: Run `cargo test --lib settings`** → Expected: compile failure (missing fields/struct).
- [ ] **Step 3: Implement** the fields, `ProviderStatus`, `load_status`, `save_status`.
- [ ] **Step 4: Run `cargo test --lib settings`** → Expected: 5 passed.
- [ ] **Step 5: Commit** `git commit -am "feat(settings): provider choice, Claude Code path, provider status cache"`

---

### Task 4: Claude Code provider module

**Files:**
- Create: `src-tauri/src/ai/claude_code.rs`; register in `src-tauri/src/ai/mod.rs`
- Modify: `src-tauri/src/ingest/mod.rs` (`Extracted::Pdf` gains `path: String`; `extract_file` fills it; update the two existing constructors in tests and in `prompt.rs` tests), `src-tauri/src/ai/prompt.rs` (add `user_text`)
- Test: `claude_code.rs` `#[cfg(test)]`, `prompt.rs` tests

**Interfaces:**
- Consumes: `ExtractionRequest`, `SYSTEM_PROMPT`, `context_block`, `operations_schema()`, `ModelResponse`, `AiUsage` (from `ai::client`).
- Produces in `prompt.rs`: `pub fn user_text(req: &ExtractionRequest, pdf_paths: &[(String, String)]) -> String` — `context_block(req)`, then for each `Extracted::Text` the `<input name="…">…</input>` block as `build_user_content` writes it, then for each `(filename, path)` in `pdf_paths` the line `Read the PDF file at <path> with the Read tool; it is the input named "<filename>".`, then `Produce the operations JSON.` Each part separated by a blank line.
- Produces in `claude_code.rs`:
  - `pub enum CcError { NotFound, SignedOut, ApiKeyLogin, RateLimited, Failed(String), NoStructuredOutput, Timeout, BadJson(String), Io(String) }` with `Display` texts: NotFound "Claude Code is not installed, or Unpack cannot find it. Set the path in Settings."; SignedOut "Claude Code is signed out. Run `claude` in a terminal and sign in."; ApiKeyLogin "Claude Code is signed in with an API key, not your Claude account, so it would bill per token. Run `claude` and sign in with your Claude account, or pick the API key provider here."; RateLimited "Your Claude seat is rate-limited right now. Try again in a few minutes or switch to the API key in Settings."; Failed(s) → s; NoStructuredOutput "Claude Code returned no structured result."; Timeout "Claude Code took too long."; BadJson(s) "Claude Code's answer could not be read: {s}"; Io(s) → s.
  - `pub fn locate(override_path: &str) -> Option<PathBuf>`: override verbatim if non-empty and exists; else each `PATH` dir × [`claude.exe`, `claude.cmd`, `claude`]; else `%USERPROFILE%\.local\bin` and `%LOCALAPPDATA%\Programs\claude` with the same names.
  - `pub fn command(exe: &Path) -> tokio::process::Command`: `cmd /c <exe>` when the extension is `cmd` or `bat`, else the exe; `env_remove` the three variables; `creation_flags(0x08000000)` on Windows; `kill_on_drop(true)`.
  - `#[derive(Deserialize)] pub struct AuthStatus { pub logged_in: bool, pub auth_method: String, pub email: String, pub org_name: String, pub subscription_type: String }` (serde rename camelCase, all with defaults) and `pub fn parse_auth_status(json: &str) -> Result<AuthStatus, CcError>` (BadJson on failure).
  - `pub fn readiness(status: &AuthStatus) -> Result<String, CcError>`: `Ok("Signed in as {email} ({org_name}, {subscription_type} plan)")` when `logged_in && auth_method == "claude.ai"`; `Err(ApiKeyLogin)` when logged in otherwise; `Err(SignedOut)` otherwise.
  - `pub struct Envelope { pub structured_output: Option<Value>, pub usage: AiUsage, pub model: String }` and `pub fn parse_envelope(stdout: &str) -> Result<Envelope, CcError>`: parse the last JSON object line whose `type == "result"`; `is_error == true` → `api_error_status` 401 → SignedOut, 429 → RateLimited, else Failed(`result` text or "Claude Code reported an error"); input tokens = `usage.input_tokens + cache_read_input_tokens + cache_creation_input_tokens`; model = first key of `modelUsage` or `""`.
  - `pub async fn auth_status(exe: &Path) -> Result<AuthStatus, CcError>` runs `auth status`, 20 s timeout.
  - `pub struct CcRun { pub exe: PathBuf, pub model: String, pub effort: String, pub scratch_dir: PathBuf, pub timeout: Duration }` (callers pass `Duration::from_secs(300)`; tests pass 1 s).
  - `pub async fn run_extraction(run: &CcRun, req: &ExtractionRequest) -> Result<(ModelResponse, AiUsage), CcError>`: copies each `Extracted::Pdf` to `scratch_dir/<uuid>/<filename>`; args `-p --output-format json --json-schema <schema> --system-prompt <SYSTEM_PROMPT> --model <model> --effort <effort> --no-session-persistence --disable-slash-commands --permission-prompts none --strict-mcp-config` plus `--tools ""` when there are no PDFs, else `--tools Read --add-dir <folder>`; `current_dir(scratch_dir)`; writes `user_text` to stdin then closes it; 300 s timeout → kill → Timeout; removes the temp folder in every exit path; `structured_output` None → NoStructuredOutput; deserialises into `ModelResponse` (BadJson).
  - `pub async fn test_connection(run: &CcRun) -> Result<String, CcError>`: prompt "Reply with OK.", schema `{"type":"object","properties":{"ok":{"type":"boolean"}},"required":["ok"],"additionalProperties":false}`, returns `Envelope.model` or the configured model when empty.

- [ ] **Step 1: Write the failing pure tests** in `claude_code.rs`: `auth_status_parses_claude_ai_login` (fixture JSON from the spec's constraints → readiness Ok text "Signed in as a@example.com (Example Org, team plan)"), `auth_status_with_api_key_login_is_not_ready` (`authMethod: "apiKey"` → `ApiKeyLogin`), `auth_status_logged_out` → `SignedOut`, `envelope_success_yields_structured_output_and_usage` (fixture with `input_tokens: 2, cache_creation_input_tokens: 70060, cache_read_input_tokens: 0, output_tokens: 150` → input 70062, output 150, model `claude-haiku-5-5`), `envelope_error_401_maps_to_signed_out`, `envelope_error_429_maps_to_rate_limited`, `envelope_without_structured_output_is_error`, `command_uses_cmd_for_cmd_shims` (a `.cmd` path → program is `cmd`), `locate_prefers_override`; in `prompt.rs`: `user_text_lists_inputs_and_pdf_paths` (one text input and one pdf path → contains `<input name="a.txt">`, the Read line, ends with `Produce the operations JSON.`), `api_body_unchanged_by_user_text` (`build_body` output equals the value asserted by the existing body test).
- [ ] **Step 2: Run `cargo test --lib claude_code` and `cargo test --lib prompt`** → Expected: compile failure.
- [ ] **Step 3: Implement** `claude_code.rs` pure parts, `user_text`, and the `Extracted::Pdf.path` change.
- [ ] **Step 4: Run both** → Expected: all pass.
- [ ] **Step 5: Write the failing spawn tests** using a fake CLI: a helper `fake_claude(dir, envelope_json) -> PathBuf` writes `claude.cmd` containing `@echo off` + `type "<dir>\envelope.json"` (and, for the stdin test, `@echo off` + `more > "<dir>\stdin.txt"` before printing). Tests: `fake_cli_run_returns_model_response` (envelope with a valid operations `structured_output` → `ModelResponse` with one create), `signed_out_envelope_maps_to_signed_out` (`is_error: true, api_error_status: 401`), `large_prompt_goes_through_stdin` (a 200,000-char text input; the fake writes stdin to a file; assert the file is ≥ 200,000 bytes), `pdf_temp_folder_is_removed_after_run` (one `Extracted::Pdf` whose `path` is a small real file; after the run the only entries under `scratch_dir` are none), `timeout_kills_the_child` (a fake that runs `ping -n 10 127.0.0.1 >nul`; `CcRun.timeout` 1 s → `Timeout` within 3 s). Tests are `#[cfg(windows)]` and use `tokio::test`.
- [ ] **Step 6: Run `cargo test --lib claude_code`** → Expected: the five spawn tests fail (functions missing or wrong results).
- [ ] **Step 7: Implement** `command`, `auth_status`, `run_extraction`, `test_connection`.
- [ ] **Step 8: Run `cargo test --lib`** → Expected: all pass (previous count + 16).
- [ ] **Step 9: Add the gated real-CLI test** `real_cli_test_connection_when_signed_in`: skipped with a printed note unless `locate("")` finds the CLI and `auth_status` reports ready; otherwise asserts `test_connection` returns a non-empty model. Run it once here → Expected: PASS on this machine.
- [ ] **Step 10: Commit** `git commit -am "feat(ai): Claude Code provider: locate, auth probe, headless extraction"`

---

### Task 5: Provider dispatch in commands

**Files:**
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` (register `probe_provider`)
- Test: `commands.rs` `#[cfg(test)]` for the pure helpers

**Interfaces:**
- Consumes: Task 3 `Settings.provider`, `ProviderStatus`, `load_status/save_status`; Task 4 `locate`, `auth_status`, `readiness`, `CcRun`, `run_extraction`, `test_connection`.
- Produces: `pub enum Provider { ClaudeCode, ApiKey }` with `impl Provider { pub fn parse(s: &str) -> Option<Provider> }` ("claude_code" | "api_key"); `pub async fn probe_provider(state) -> CmdResult<ProviderStatus>`; `SettingsView` gains `pub provider_status: ProviderStatus`; `AppState` gains `pub status: Mutex<ProviderStatus>`.
- Behaviour: `probe_provider` resolves the provider (`settings.provider` unset → probe Claude Code; ready → save `provider = "claude_code"`, else `"api_key"`), computes the status (`claude_code`: `locate` → NotFound detail, else `auth_status` → `readiness`; `api_key`: ready iff a key exists, detail "Key saved" / "Add your API key"), stores it in `AppState.status` and the settings table, returns it. `start_capture`: `Provider::ClaudeCode` → `claude_code::run_extraction(&CcRun { exe: locate(..) or Err(NotFound), model, effort, scratch_dir: data_dir/tmp })`, `ApiKey` → unchanged; both results map into the single existing `resolve` call. `test_connection` branches the same way. `get_settings` returns the cached status. On launch, `lib.rs` `setup` removes `data_dir/tmp` if it exists.

- [ ] **Step 1: Write failing tests** `provider_parse_accepts_two_values_only`, `unset_provider_defaults_by_readiness` (pure helper `choose_provider(unset: bool, current: &str, cc_ready: bool) -> &'static str`: unset+ready → "claude_code", unset+not ready → "api_key", set → unchanged).
- [ ] **Step 2: Run `cargo test --lib commands`** → Expected: compile failure.
- [ ] **Step 3: Implement** the enum, helpers, `probe_provider`, the branches, `SettingsView.provider_status`, `AppState.status`, the launch cleanup; register the command.
- [ ] **Step 4: Run `cargo test --lib` and `cargo clippy --all-targets -- -D warnings`** → Expected: all pass, clippy clean.
- [ ] **Step 5: Commit** `git commit -am "feat: provider dispatch, readiness probe, Claude Code test connection"`

---

### Task 6: Settings UI, rail badge, capture pre-flight

**Files:**
- Modify: `src/lib/types.ts` (`Settings.provider`, `Settings.claude_code_path`, `ProviderStatus`, `SettingsView.provider_status`), `src/lib/api.ts` (`probeProvider`, mock map entry), `src/lib/mockApi.ts` (provider state, `probeProvider`, provider-aware `testConnection`/`startCapture`), `src/store/useStore.ts` (`probeProvider` action called from `loadSettings`), `src/views/SettingsView.tsx`, `src/components/Rail.tsx`, `src/views/CaptureSheet.tsx`
- Test: `src/store/provider.test.ts`

**Interfaces:**
- Produces: `src/store/provider.ts` with `export function providerReady(view: SettingsView | null): boolean` (null → true, else `view.provider_status.ready`) and `export function notReadyMessage(view: SettingsView | null): string` (`claude_code` → `view.provider_status.detail`; `api_key` → "Add your Anthropic API key in Settings first."). Rail and CaptureSheet use these instead of `has_key`.
- Settings "Claude" section per the spec: radios `Claude Code (uses your Claude subscription)` / `API key (pay per use)` saving `provider` on change then calling `probeProvider`; under Claude Code: the status line (`account` when ready, `detail` otherwise, with the "ok-text"/"error" classes), **Re-check**, **Test connection**, a collapsed "Claude Code path" input saved on blur; under API key: the existing rows. Model and Effort stay below.

- [ ] **Step 1: Write failing tests** in `provider.test.ts`: `ready_when_status_ready`, `claude_code_detail_is_the_message`, `api_key_message_is_fixed`.
- [ ] **Step 2: Run `npx vitest run src/store/provider.test.ts`** → Expected: FAIL, module missing.
- [ ] **Step 3: Implement** `provider.ts`, types, api, mock, store, views.
- [ ] **Step 4: Run `npm test` and `npm run build`** → Expected: 25 passed; build passes.
- [ ] **Step 5: Verify in the browser** (`npm run dev`, mock mode): Settings shows the provider radios; switching to Claude Code shows "Signed in as sam@example.com (Example Org, team plan)"; Re-check and Test connection work; the capture pre-flight message changes with the provider.
- [ ] **Step 6: Commit** `git commit -am "feat(ui): provider choice in Settings; provider-aware badge and capture check"`

---

### Task 7: Custom title bar

**Files:**
- Modify: `src-tauri/tauri.conf.json` (`decorations: false`, `shadow: true`), `src-tauri/capabilities/default.json` (add `core:window:allow-minimize`, `core:window:allow-toggle-maximize`, `core:window:allow-close`, `core:window:allow-is-maximized`), `src/App.tsx`, `src/styles/components.css`
- Create: `src/components/TitleBar.tsx`

**Interfaces:**
- Produces: `TitleBar()` renders `<header class="titlebar" data-tauri-drag-region>` 36 px high: left `<span class="tb-brand">` with the icon (`/src-tauri/icons/32x32.png` copied to `public/icon-32.png`) and "Unpack"; right three `<button class="tb-btn">` (Minus, Square/Copy when maximized, X icons at 14 px) 46 px wide, the close button with class `tb-close`. Handlers: `getCurrentWindow().minimize()`, `.toggleMaximize()`, `.close()`; double-click on the header toggles maximize; all guarded by `inTauri`. `App` layout becomes `grid-template-rows: 36px 1fr` with the existing columns below the bar; detail panel and sheets stay below the bar (`.backdrop` top offset 36 px).

- [ ] **Step 1: Implement** the config, capabilities, component and CSS (`.titlebar` background `var(--canvas)`, border-bottom `var(--border)`, `.tb-btn:hover` background `var(--surface)`, `.tb-close:hover` background `var(--danger)` with white icon; `-webkit-app-region` is not used, the drag attribute is).
- [ ] **Step 2: Verify in the browser** (mock): the bar renders above the rail with the brand and three controls; nothing overlaps.
- [ ] **Step 3: Verify in the real app**: `npm run tauri dev`, then from PowerShell capture the screen (`Add-Type -AssemblyName System.Drawing; [System.Windows.Forms.Screen]`-based `CopyFromScreen` to `<scratchpad>/titlebar.png`) and view the PNG → Expected: no native title bar, the custom bar with working close.
- [ ] **Step 4: Run `npm run build`** → Expected: passes.
- [ ] **Step 5: Commit** `git commit -am "feat(ui): custom title bar, no native window chrome"`

---

### Task 8: Headless mode

**Files:**
- Create: `src-tauri/src/headless.rs`
- Modify: `src-tauri/src/lib.rs` (run headless before building the window), `README.md` (a "Command line" section)
- Test: `headless.rs` `#[cfg(test)]`

**Interfaces:**
- Consumes: `db::open`, `settings`, `captures::{insert_capture, insert_input, set_proposal}`, `tasks::list_tasks`, `resolve`, `apply_proposal`, Task 5 probe logic (factored into `pub async fn compute_status(conn: &Connection, data_dir: &Path) -> ProviderStatus` in `commands.rs` so headless and the command share it).
- Produces: `pub fn maybe_run(identifier: &str, args: &[String]) -> Option<i32>`: returns `None` when `args[1]` is not one of `--probe`, `--list-tasks`, `--import`, `--set-status`; otherwise runs the mode and returns the exit code. Modes: `--probe --out <file>`, `--list-tasks --out <file>`, `--import <proposal.json> --out <file> [--as <label>]`, `--set-status <task_id> <todo|in_progress|done> --out <file>` (the same `tasks::update_task` call the status dropdown makes; output `{"ok": true, "task": Task}`). Data dir = `%APPDATA%\<identifier>`. Output file JSON: `{"ok": true, "probe": ProviderStatus}` | `{"ok": true, "tasks": [Task]}` | `{"ok": true, "capture_id": "...", "changes": [Change], "warnings": [String]}` | `{"ok": false, "error": "..."}`. Also `pub fn import_proposal(conn: &Connection, json: &str, filename: &str, label: &str) -> Result<(String, Vec<Change>, Vec<String>), String>` (the testable core: parse `ModelResponse`, insert capture with `model = label`, insert one `file` input, resolve against open tasks, `set_proposal`, apply all indices).

- [ ] **Step 1: Write failing tests** `import_creates_tasks_and_records_capture` (two creates, one a child via temp id → 2 tasks, capture `applied`, `model == "manual-import"`), `import_rejects_bad_json` (Err with "could not be read"), `set_status_updates_the_task` (pure core `set_status(conn, id, "in_progress")` → task status `InProgress`; unknown status → Err), `maybe_run_ignores_normal_launch` (`["unpack.exe"]` → None).
- [ ] **Step 2: Run `cargo test --lib headless`** → Expected: compile failure.
- [ ] **Step 3: Implement** `headless.rs`; in `lib.rs` `run()`: `let context = tauri::generate_context!(); let args: Vec<String> = std::env::args().collect(); if let Some(code) = headless::maybe_run(&context.config().identifier, &args) { std::process::exit(code); }` before the builder; `--probe` uses `tauri::async_runtime::block_on`.
- [ ] **Step 4: Run `cargo test --lib` and clippy** → Expected: pass, clean.
- [ ] **Step 5: Verify once for real**: `cargo build` then `target/debug/unpack.exe --list-tasks --out <scratchpad>/t.json` → Expected: `{"ok":true,"tasks":[...]}`.
- [ ] **Step 6: README section** "Command line" documenting the four modes and `--out`.
- [ ] **Step 7: Commit** `git commit -am "feat: headless modes: --probe, --list-tasks, --import"`

---

### Task 9: Docs and full verification

**Files:**
- Modify: `README.md` (providers section: Claude Code first, API key second, how readiness is decided, the data folder `%APPDATA%\com.parth.unpack`, the title bar note), `docs/superpowers/HANDOFF.md` (state after this plan)

- [ ] **Step 1: Write the README and HANDOFF changes.**
- [ ] **Step 1b: Write the project memory entry** (outside the repo) in Claude Code's memory directory for this repo: a `project` memory `unpack-app` saying the app is named Unpack (formerly PGTaskTracker), lives in this folder, uses the Claude Code provider by default, has headless modes, and that company context must never enter the repo; add its line to `MEMORY.md`.
- [ ] **Step 2: Run** `cd src-tauri && cargo test --lib && cargo clippy --all-targets -- -D warnings`, `npm test`, `npm run build`, the rename check and the scrub check → Expected: all pass, both checks exit 0.
- [ ] **Step 3: Commit** `git commit -am "docs: Unpack README and handoff"`

> After Task 9 the executing skill's final whole-branch review runs (fresh reviewer, most capable model, this plan's Review Focus, the ledger's rulings). Fix Critical/Important with TDD, then continue with the operational tasks below.

---

### Task 10: Build, install, configure on this PC

- [ ] **Step 1: `npm run tauri build`** → Expected: `src-tauri/target/release/bundle/nsis/Unpack_0.2.0_x64-setup.exe` (bump `version` to `0.2.0` in `tauri.conf.json`, `Cargo.toml`, `package.json` first, committed as `chore: v0.2.0`).
- [ ] **Step 2: Install** for the current user: run the NSIS installer with `/S` (silent; Tauri's per-user default folder is `%LOCALAPPDATA%\Unpack`) → Expected: `unpack.exe` exists there; record the path in the ledger.
- [ ] **Step 3: Probe**: `unpack.exe --probe --out <scratchpad>/probe.json` → Expected: `provider: "claude_code"`, `ready: true`, `account` starts with "Signed in as".
- [ ] **Step 4: Launch** the installed app, capture the screen as in Task 7 → Expected: custom title bar, Focus view, no error toast. Close it.
- [ ] **Step 5: Ledger** the install path and the probe result.

---

### Task 11: Publish a clean snapshot to GitHub

- [ ] **Step 1: Ask the user** to sign the personal account in: `! gh auth login` (GitHub.com, HTTPS, browser) then `gh auth switch --user <personal login>`; wait for it.
- [ ] **Step 2: Snapshot**: on the merged `master` tree, `git checkout --orphan snapshot && git add -A && git commit -m "Unpack v0.2.0"` with the personal author email; run the scrub check and the rename check on that tree → Expected: both exit 0.
- [ ] **Step 3: Create and push**: `gh repo create Unpack --private --source . --push` with the `snapshot` branch pushed as `main`; set the local `master` to track nothing new. Report the URL.
- [ ] **Step 4: Switch back** to `master`; keep `snapshot` locally.

---

### Task 12: Load the user's real tasks from the planning workspace

- [ ] **Step 1: Read** `<workspace>/Progress_Tracker/03_My_Progress_Report.md`, `01_Manager_Action_List.md`, `action-list.md`, `Asks_Report.md`, `README.md`; each task folder's progress file; the newest file in each task folder's `Meetings/`; the ticket index. Read-only.
- [ ] **Step 2: Compose** `<scratchpad>/import-1.json` in the `ModelResponse` shape: one `create` per piece of work (category = folder name, priority from the agreed order of work, due dates only where the notes state one, description one or two sentences, evidence a short quote), child `create`s with `parent_id = <temp_id>` for the concrete open items, `reason` on every op.
- [ ] **Step 3: Import**: `unpack.exe --import <scratchpad>/import-1.json --out <scratchpad>/r1.json --as "session-import"` → Expected: `ok: true`, zero warnings (fix the JSON and re-run otherwise; undo any partial import from Activity if needed).
- [ ] **Step 4: Compose and import** `import-2.json` with `complete` ops for the items the notes mark done (task ids from `--list-tasks`); then `--set-status <id> in_progress` for each item the notes say is under way.
- [ ] **Step 5: Verify**: `--list-tasks` → counts by category and status; open the app and check Focus.
- [ ] **Step 6: Report** to the user: what was loaded, what was marked done, what was uncertain. Nothing is written to the workspace or the repo.
