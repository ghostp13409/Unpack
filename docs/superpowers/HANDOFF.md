# Unpack — session handoff (2026-11-01, after the rename and Claude Code provider)

Written so a fresh Claude Code session can pick this up without the previous conversation.

## What this is

A Tauri 2 (Rust + React/TypeScript) desktop task tracker named **Unpack** (v0.1.0 shipped under the old name; see the README). The user drops context (Teams .vtt transcripts, .eml emails, .docx, .pdf, .md/.txt, or pasted text) and Claude proposes task operations (create / update / complete / add note) which the user reviews and applies. A deterministic "Up next" ranking drives the Focus view. Two providers reach Claude: **Claude Code** run headlessly (the user's Claude subscription; the default when it is signed in) and an **API key**.

Read these first, in order:
1. `docs/overview.md` — the user's original brief.
2. `docs/superpowers/specs/2026-11-01-unpack-v1-design.md` — the v0.1.0 design.
3. `docs/superpowers/specs/2026-11-01-unpack-rename-and-claude-code-design.md` — the rename, the Claude Code provider, the custom title bar, headless modes, and (addendum) the GitHub, install and task-loading requests.
4. `docs/superpowers/plans/2026-11-01-unpack-rename-and-claude-code.md` — the 12-task plan (9 code tasks, then review, then operational tasks 10–12).
5. `README.md` — how to run, test, build; providers; command line; where data lives.

Process used: superpowers skills (`executing-plans` inline, TDD per task, one fresh whole-branch review at the end). Branch: `feat/v1`, started from `master` at the docs-only commit `98bcfe5`; the v1 work and this work are both on it, unmerged. The git-ignored execution ledger lives under `.superpowers/sdd/`; `git log` is the durable record.

## Hard rule: no company context in the repo

The user's instruction: anything learned from their work context stays in the installed app's database only. Mock data, test fixtures and docs use fictional names (Sam, Alex Morgan, Jordan; Example Org; categories Platform, Observability, Pricing, Cloud, Admin; projects Atlas, Beacon). A denylist and two check scripts live **outside** the repo in the session scratchpad; the final-review and the GitHub steps run them. GitHub receives a single clean snapshot commit, never the local history.

## State

Code tasks 1–9 of the plan are done. Verification at HEAD: `cargo test --lib` 108 passed · `cargo clippy --all-targets -- -D warnings` clean · `npm test` 25 passed · `npm run build` passes. Verified by hand: the Claude Code provider against the real signed-in CLI (gated test `real_cli_test_connection_when_signed_in` and `unpack --probe`), the custom title bar in the real window (no native chrome; close control exits), the Settings provider section in the browser mock.

Done after the final review (2026-11-01 evening): the review's Critical and Important findings fixed as minimal edits (the user asked for no test ceremony after the review); version 0.2.0 built and installed for the current user at `%LOCALAPPDATA%\Unpack\unpack.exe`; the installed app's `--probe` reports the Claude Code provider ready with the user's work Claude account; a clean single-commit snapshot of the tree pushed to a private GitHub repo named Unpack under the user's personal account (default branch `main`; the local branch `snapshot` tracks it; the full history stays local on `feat/v1`); the user's real tasks loaded into the installed app's database from the planning workspace through `unpack --import` (97 tasks, 8 done, 14 in progress) with the proposal files kept outside the repo. Not verified in the installed build: the title bar double-click (handled by Tauri's drag script after the review fix). `feat/v1` is still unmerged into `master`; that decision is the user's.

## MCP server (v0.3.0, 2026-11-02, branch `feat/mcp`)

`unpack --mcp` serves the Model Context Protocol over stdin/stdout (newline-delimited JSON-RPC, hand-written in `src-tauri/src/mcp.rs`, no new dependencies). It exposes every app feature except setting or clearing the API key (secrets stay out of chat) and opening the data folder (a window action): tasks, notes, categories, up next, capture/apply/discard, `apply_operations` (Claude Code's own proposal recorded as an undoable capture, built on `headless::import_response`), activity, undo, settings and the provider check. Shared logic moved out of the Tauri commands into `task_detail`, `run_capture`, `apply_selected`, `capture_summaries` and `headless::probe`. `db::open` sets a 5 s busy timeout because the window and the MCP server write the same database, and the window reloads tasks and settings on focus. Registered for the user with `claude mcp add --scope user unpack -- <exe> --mcp`.

## Rulings worth knowing

- reqwest uses `native-tls` plus `system-proxy` (Windows cert store and proxy settings).
- Claude Code is spawned with `-p --output-format json --json-schema … --system-prompt … --tools "" --no-session-persistence --disable-slash-commands --permission-prompts none --strict-mcp-config`, the user prompt on stdin, `ANTHROPIC_API_KEY`/`ANTHROPIC_AUTH_TOKEN`/`ANTHROPIC_BASE_URL` removed, `CREATE_NO_WINDOW`, 300 s timeout. Never `--bare` (it disables OAuth). PDFs are copied to `<data>/tmp/<uuid>/` and exposed with `--tools Read --add-dir`; the folder is removed after the call and `<data>/tmp` is wiped at launch.
- "Ready" = `claude auth status` with `loggedIn: true` and `authMethod == "claude.ai"`. An npm `claude.cmd` shim is resolved to `node <cli.js>` and node is spawned directly, because Windows cannot pass the quoted schema argument to a batch file (any other batch file fails with a message to set the path to `claude.exe`); the native `claude.exe` is what this PC has.
- `compute_status(settings, has_key)` takes a settings snapshot so no DB lock is held across the probe.
- Headless modes write JSON to `--out` because the release exe is a GUI-subsystem binary with no console.
- Browser mock mode (`src/lib/mockApi.ts`) is active only when `__TAURI_INTERNALS__` is absent.
- Keyboard: pure logic in `src/hooks/shortcuts.ts`, DOM glue in `useShortcuts.ts`; J/K cursor is `cursorId` in the store.

## How to run

```
npm install
npm run dev            # browser preview with mock data (no Rust needed)
npm run tauri dev      # real app; kill anything on port 5173 first (strictPort)
npm test               # vitest
cd src-tauri && cargo test --lib && cargo clippy --all-targets -- -D warnings
npm run tauri build    # installers under src-tauri/target/release/bundle/
```

Gotchas learned: the Bash tool in this harness truncates long heredocs (roughly >6 KB) and Python heredocs choke on backslash escapes, so patch scripts are written with the Write tool (raw strings) and run from the scratchpad. `keyring` 4 uses feature `windows-native-keyring-store`; `reqwest` 0.13 needs `system-proxy` listed explicitly when `default-features = false`. httpmock 0.9 renamed `json_body_partial` to `json_body_includes`. Taking `&lock(&state)?` needs `&*lock(&state)?`. Vite binds `[::1]:5173` only, so use `http://localhost:5173`. Stopping a background `npm run dev` task does not kill node; kill the PID on port 5173. A screen capture of the real window works from PowerShell (`CopyFromScreen`) and the PNG can be viewed with the Read tool.
