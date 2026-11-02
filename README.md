# Unpack
Formerly PGTaskTracker (v0.1.0).

A personal desktop task tracker that turns context into tasks. Drop in Teams
transcripts (`.vtt`), emails (`.eml`), Word documents, PDFs, Markdown or plain
text, or just paste a few lines, and Claude proposes task operations: create,
update, complete, add a note. You review the proposal, apply it, and can undo
any change later from the Activity view. A deterministic "Up next" ranking
drives the Focus view so the first thing you see is what to work on.

Built with Tauri 2 (Rust) and React + TypeScript. Windows is the primary target.

## Requirements

- Node.js 20 or newer
- Rust stable (1.90 or newer) via rustup
- Windows: Visual Studio Build Tools with the C++ workload, and the WebView2
  runtime (already present on Windows 10/11)
- Either Claude Code installed and signed in with your Claude account, or an
  Anthropic API key (entered once, inside the app)

## Run it

```
npm install
npm run tauri dev      # the real app; the first Rust build takes several minutes
npm run tauri build    # installer under src-tauri/target/release/bundle/
```

`npm run dev` alone starts the UI in a browser with in-memory mock data, which
is handy for working on views without the Rust side. The mock is never active
inside the Tauri shell.

Tests:

```
npm test                                   # vitest (frontend logic)
cd src-tauri && cargo test --lib           # Rust: parsers, ranking, apply/undo, API client
cd src-tauri && cargo clippy --all-targets -- -D warnings
```

## Two ways to reach Claude

Settings has a **Provider** choice:

- **Claude Code** (the default when it is available): Unpack runs the `claude`
  command on this PC in its print mode, with the same prompt and the same JSON
  schema the API path uses, so your Claude subscription does the work and
  nothing is billed per token. "Ready" means Claude Code is installed and
  signed in with a Claude account (a Console or API-key login inside Claude
  Code does not count, so the app can never spend API money silently). If the
  command is not on your `PATH`, set its location under **Set the path**.
- **API key**: pay per use through the Anthropic API. The key lives in Windows
  Credential Manager.

Both providers show their state in Settings with **Re-check** and
**Test connection**; the settings badge on the rail turns on when the selected
provider is not ready. There is no automatic fallback from one to the other.

## First launch

1. Open Settings (Ctrl+,). If Claude Code is signed in, the status line reads
   "Signed in as …" and you are done; otherwise pick **API key** and paste one.
   Press **Test connection** either way.
2. Set your name, and your manager's name if you want their asks to default to
   high priority.
3. Press Ctrl+N (or the + button), drop files or paste text, and press
   Ctrl+Enter. Review the proposal and apply it.

A single short line in the Capture sheet followed by plain Enter creates a task
directly without calling the model.

## Keyboard

| Keys | Action |
| --- | --- |
| Ctrl+N | Open the Capture sheet |
| Ctrl+K | Jump to Tasks and focus search |
| Ctrl+, | Settings |
| J / K | Move the cursor down / up the visible list |
| Enter | Open the task under the cursor |
| D | Toggle done on the task under the cursor (or the open task) |
| E | Edit the open task's title |
| Esc | Leave a text field, then close the detail panel or sheet |

Letter shortcuts are ignored while you are typing in a field.

## Command line

The installed `unpack.exe` also works without the window, for scripts and for
loading tasks in bulk. Results are written to the `--out` file as JSON
(`{"ok": true, ...}` or `{"ok": false, "error": "..."}`); the exit code is 0 or 1.

```
unpack --probe --out status.json                 # provider readiness
unpack --list-tasks --out tasks.json             # every task, done ones included
unpack --import proposal.json --out r.json       # apply a proposal file
unpack --set-status <task id> in_progress --out r.json
```

A proposal file has the model's response shape, `{"summary": "...",
"operations": [...]}`, with every operation field present (use `null` for
the ones that do not apply). It is stored as a capture, resolved against your
open tasks exactly like a model answer, applied in full, and shows up in
Activity where each change can be undone. `--as <label>` sets the capture's
model label (default `manual-import`).

## Where data lives

- Tasks, notes, captures and change history are in a SQLite database at
  `%APPDATA%\com.parth.unpack\tasks.db`. Settings → **Open folder**
  takes you there. Back up that one file to keep everything.
- The API key is stored in Windows Credential Manager through the `keyring`
  crate. It is never written to the database or to any file, and Settings
  only ever shows its last four characters. Unpack never reads Claude Code's
  own login tokens; it only runs the `claude` executable.
- Nothing is sent anywhere except to Claude (through Claude Code or the API),
  and only when you process a capture or test the connection. Each request
  carries the inputs you dropped plus a compact list of your open tasks so the
  model can update existing ones. PDFs on the Claude Code provider are copied
  into a per-capture folder under the data directory and removed afterwards.
- The window draws its own title bar; there is no native chrome.

## Layout

```
src/                 React UI (views, components, zustand store, API layer)
src/lib/mockApi.ts   browser-only stand-in for the backend
src-tauri/src/       Rust: models, SQLite repo, ranking, ingest parsers,
                     Claude client + prompt, apply/undo, Tauri commands
docs/                original brief, design spec and implementation plan
```
