# Unpack: rename and Claude Code provider — design

Date: 2026-11-01. Builds on `2026-11-01-unpack-v1-design.md` (the v0.1.0 design) and the shipped `feat/v1` branch. Everything in that design still holds unless this document says otherwise. Items marked **(my call)** are builder decisions the user has not confirmed.

## Intent

Two changes to the finished v0.1.0 app:

1. **Rename the product from PGTaskTracker to Unpack**, everywhere the old name appears: code, config, installers, docs, the repo folder, and Claude Code's project memory. The user's words: "it's a complete refactor".
2. **Let the app use the user's Claude Teams seat** by running Claude Code headlessly, while keeping the existing API-key path as a second option.

Why: the user has a Claude Teams account with Claude Code installed and signed in on the PC where the app runs, and does not want to pay per token through an API key when the seat already covers it. A Teams seat has no public API a desktop app may call directly, and Anthropic does not permit third-party apps to borrow Claude Code's login tokens. Running Claude Code itself in print mode (`claude -p`) is the sanctioned route and it supports exactly what the app needs: one prompt in, one schema-validated JSON object out.

## Goals

- A user with Claude Code signed in can install Unpack, open Settings, see "Claude Code: signed in as …", and process a capture with no API key.
- The API-key provider keeps working exactly as today.
- Review, Apply, Undo, ranking and every view are provider-agnostic. The provider only changes how `ModelResponse` is obtained.
- Nothing in the repo, docs, installers, window chrome or memory still says PGTaskTracker (git history excepted; it is not rewritten).

## Non-goals

- Phone or web access. Out of scope by the user's instruction.
- Automatic fallback from one provider to the other.
- Migrating data from the old `%APPDATA%\com.parth.pgtasktracker` folder (the only database there is the empty one created by `tauri dev`).
- Streaming, multi-turn, or tools inside the extraction call. One request, one structured result, as today.

## Anti-goals (would make this a failure even if it works)

- Spending the user's API-key money without telling them: no silent switch to the API key when Claude Code fails.
- Reading or storing Claude Code's OAuth tokens. The app only ever runs the `claude` executable.
- Leaving the two providers with different extraction quality: both must send the same system prompt, the same inputs, and validate against the same JSON schema.
- Any remaining "PGTaskTracker" or "pgtasktracker" string in the tree after the rename (except git history).

## Constraints

- Verified on this machine: Claude Code 2.1.316 at `~/.local/bin/claude`; `claude auth status` prints JSON with `loggedIn`, `authMethod` ("claude.ai"), `email`, `orgName`, `subscriptionType` ("team"). A headless run of `claude -p "<prompt>" --output-format json --model haiku --tools "" --no-session-persistence --disable-slash-commands --system-prompt "…" --json-schema '{…}'` returned one JSON envelope with `type:"result"`, `subtype:"success"`, `is_error:false`, `structured_output:{…}` (the validated object), `result` (the same as a string), `usage`, `modelUsage`, `api_error_status`, `num_turns`, `duration_ms`.
- `--bare` must not be used: it disables OAuth and would defeat the purpose.
- Windows command lines are limited to roughly 32 K characters; transcripts are far bigger. The user prompt goes to Claude Code on stdin, never on argv.
- Spawned processes on Windows must not flash a console window (`CREATE_NO_WINDOW`).
- The v0.1.0 global constraints stay: API key only in Credential Manager; enums snake_case; the 350,000-character text guard; the per-file size caps; plain CSS tokens.
- Process in use: superpowers (`writing-plans`, then `executing-plans` inline with TDD and one fresh whole-branch review).

## Decisions

### Provider model

- A new setting `provider` with values `claude_code` | `api_key`, stored in the `settings` table like the others.
- **Default (my call):** on the first launch after this change (or whenever `provider` is unset), the backend probes Claude Code; if the executable is found and `auth status` reports `loggedIn: true`, the provider is `claude_code`, otherwise `api_key`. The probe result is persisted so the choice is stable afterwards; the user can change it in Settings at any time.
- Both providers implement one Rust trait (name left to the builder, e.g. `Extractor`) with a single method that takes the existing `ExtractionRequest` and returns `(ModelResponse, Usage)`. `start_capture` picks the implementation from settings and otherwise does not change. `resolve`, `apply`, `undo` are untouched.
- The system prompt, the user-content assembly (task list, categories, today, inputs) and the operations JSON schema are shared code, used by both providers. The API provider keeps sending the schema through `output_config.format`; the Claude Code provider passes the same schema through `--json-schema`.

### Claude Code provider

- **Locating the executable (my call):** search `PATH` for `claude`, `claude.exe`, `claude.cmd` in that order, then the known install locations `%USERPROFILE%\.local\bin` and `%LOCALAPPDATA%\Programs\claude`. Settings has an optional "Claude Code path" override for a user whose install is elsewhere; when set it is used verbatim. A Tauri app launched from the Start menu may see a different `PATH` than a terminal, which is why the known locations and the override exist.
- **Sign-in probe:** run `claude auth status`, parse the JSON. "Ready" means exit 0, `loggedIn: true` **and** `authMethod == "claude.ai"`. Any other `authMethod` (a Console login or an inherited `ANTHROPIC_API_KEY`) counts as not ready with its own message: "Claude Code is signed in with an API key, not your Claude account, so it would bill per token. Run `claude` and sign in with your Claude account, or pick the API key provider here." A non-zero exit, unparsable output or `loggedIn: false` is "Not signed in. Run `claude` once in a terminal and sign in." **(my call)** Settings shows the ready result as one line: "Signed in as parth@… (Example Org Users, team plan)".
- **Child environment (my call):** every spawned `claude` process gets `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN` and `ANTHROPIC_BASE_URL` removed from its environment, so the subscription login is the only credential it can use.
- **When the probe runs (my call):** on app launch in the background (the frontend calls a `probe_provider` command right after `get_settings`), when Settings opens, and on **Re-check**. The result (`ready`, `detail`, `checked_at`) is cached in `AppState` and persisted in the `settings` table; `get_settings` stays synchronous and returns the last known result, so the rail badge and the Capture pre-flight have an answer at once and are corrected within a few seconds of launch. A capture started while the cached result is stale just runs; a real failure is reported by the error mapping below.
- **Extraction command** (builder may add flags that reduce the fixed per-call context, but must keep these):

  ```
  claude -p --output-format json --json-schema <schema> --system-prompt <text>
         --model <settings.model> --effort <settings.effort>
         --tools "" --no-session-persistence --disable-slash-commands
         --permission-prompts none --strict-mcp-config
  ```

  with the user prompt on stdin, working directory set to the app's data folder (never the user's documents), and `--tools ""` so no file or shell tool is available. The smoke test showed about 70 K cached input tokens of fixed context per call even with tools off; that costs seat rate-limit headroom, not money. The builder should measure whether `--setting-sources ""` or `--restricted` lowers it and keep whatever is lowest without re-enabling tools. **(my call)**
- **PDFs under Claude Code (my call, spiked and working):** the API provider sends PDFs as base64 document blocks; Claude Code cannot take those on stdin. For a capture that includes PDFs, the Claude Code provider copies each PDF into a fresh per-capture folder under the app data directory (`<data>/tmp/<capture id>/`), passes `--tools Read --add-dir <that folder>`, and tells the model in the prompt the exact path of each file to read. Only that folder is exposed, never the user's Documents or Downloads. The folder is deleted when the call ends, success or failure, and any leftovers under `<data>/tmp` are removed at launch. The spike on 2026-11-01 (a one-page PDF, prompt on stdin, `--tools Read --add-dir <dir> --permission-prompts none --strict-mcp-config`) returned a schema-valid `structured_output` with the PDF's content, no permission denials, in 3 turns, with about 9 K tokens of fixed context. `Extracted::Pdf` therefore carries the source path as well as the base64 (the API provider keeps using the base64).
- **Model and effort:** `settings.model` (default `claude-haiku-5-5`) is passed through `--model` as is; Claude Code accepts full model ids and aliases. `settings.effort` maps directly to `--effort`.
- **Parsing the result:** read stdout to the end; parse the envelope; success requires `type == "result"`, `is_error == false`, and a `structured_output` object, which is then deserialised into `ModelResponse` exactly as the API path does. Record `usage.input_tokens + cache_read_input_tokens + cache_creation_input_tokens` as input tokens and `usage.output_tokens` as output tokens on the capture so Activity shows the same numbers as the API path.
- **Error mapping** (plain sentences, as today): executable not found → "Claude Code is not installed, or Unpack cannot find it. Set the path in Settings."; not signed in (`api_error_status` 401, or auth status says so) → "Claude Code is signed out. Run `claude` in a terminal and sign in."; `is_error` with a rate-limit status (429) → "Your Claude seat is rate-limited right now. Try again in a few minutes or switch to the API key in Settings."; any other `is_error` → the envelope's `result` text; `structured_output` missing on success → "Claude Code returned no structured result." Non-zero exit with empty stdout → stderr, trimmed, or "Claude Code exited with code N".
- **Timeouts:** 5 minutes per call, then the process is killed and the capture marked failed with "Claude Code took too long." **(my call)**
- **No retries** at this layer: Claude Code retries transient API errors itself.
- **Test connection** in Claude Code mode runs a tiny headless request with the configured model and a one-field schema and reports the model that answered (from `modelUsage`), the same wording as the API path. The sign-in line comes from `auth status` and is shown separately.
- Nothing from Claude Code's output is logged to disk. stderr is kept in memory only for the error message.

### API-key provider

Unchanged. The existing client, retries, and tests stay. The Settings "Anthropic API key" rows are shown only when the provider is `api_key`, with the key still stored in Credential Manager under the new service name.

### Settings UI

- Section "Claude" at the top of Settings replaces the "Anthropic" header:
  - Provider: two radio buttons, "Claude Code (uses your Claude subscription)" and "API key (pay per use)".
  - Under Claude Code: the sign-in line, **Re-check**, **Test connection**, and the collapsed "Claude Code path" override.
  - Under API key: the existing key row and Test connection.
  - Model and Effort stay below, shared.
- The rail badge that today means "no API key" now means "the selected provider is not ready" (Claude Code not found or signed out, or no key).
- The Capture sheet's pre-flight check ("Add your Anthropic API key in Settings first.") becomes provider-aware with the error texts above.

### Rename inventory (everything that says PGTaskTracker)

Code and config:
- `package.json` name → `unpack`; `src-tauri/Cargo.toml` package name → `unpack`, lib name → `unpack_lib`, description; `src-tauri/src/main.rs` and `lib.rs` references to the lib crate.
- `src-tauri/tauri.conf.json`: `productName` → `Unpack`, `identifier` → `com.parth.unpack`, window title → `Unpack`.
- Keyring service name → `Unpack` (account name unchanged). A key saved under the old service is not migrated. **(my call)**
- `index.html` title, any `<title>`, README, the mock data dir string in `mockApi.ts`, the rail/window strings, CSS comments, Rust doc comments, test names and fixtures that embed the name.
- The icon stays as generated (it carries no text).

Docs: `docs/overview.md`, both specs, the plan, `HANDOFF.md`: the name in prose and headings. The two v0.1.0 files are renamed `2026-11-01-unpack-v1-design.md` and `2026-11-01-unpack-v1.md` (plan). This file is already named for Unpack. Historical wording like "formerly PGTaskTracker" is allowed once, in the README, so a reader of git history can connect the two. **(my call)**

Memory: a project memory entry for Unpack is written in Claude Code's memory directory for this repo, and the same entry is written to the directory that the renamed folder path will map to, so the next session in the new folder starts with it.

Folder: `<repo>` → `<parent>\Unpack`, done last, after the last commit. Windows refuses to rename a directory that is the current directory of any running process, and the Claude Code session doing the work runs inside it, so the session cannot rename it directly. How the rename happens is the user's choice (see the open question at the end). Either way, the session writes the memory entry for the new path before the rename and tells the user to reopen Claude Code in `<workspace>\Unpack`.

Data: the data folder moves to `%APPDATA%\com.parth.unpack` because the identifier changes. No migration; the old folder is left alone and the README says so.

## What is left to the builder

- Trait and module names; where the process spawning lives (a new `ai/claude_code.rs` next to `ai/client.rs` is the obvious shape).
- How the tests exercise the Claude Code provider without the real CLI: the obvious approach is a fake `claude` script on `PATH` (or an explicit path override) that echoes a canned envelope, plus pure tests of envelope parsing and error mapping. At least one test runs the real CLI only when it is present and signed in, and is skipped otherwise.
- The exact flag set that minimises fixed context per call, measured.
- The PDF spike's outcome and the amended wording if it fails.
- The order of the rename within the branch (one mechanical commit is fine).

## Review focus for the final review

1. A capture with the provider set to `claude_code` while Claude Code is signed out must fail with the sign-in message and leave the capture `failed`, never hang.
2. A transcript larger than the Windows argv limit must still reach Claude Code intact (stdin path).
3. The API-key path must be byte-for-byte unchanged in what it sends (existing httpmock tests still pass).
4. The two providers must produce `ModelResponse` through the same `resolve` path; a schema change in one place must affect both.
5. `grep -ri pgtasktracker` over the tree (excluding `.git`) returns nothing.

## Addendum 2026-11-01 (evening): scope added by the user after the builder check

The user approved the design above with these additions and one change: **the repo folder is not renamed** (the user: "if you cannot rename the base folder that's alright, just make sure that in the github the repo is named unpack"). The open question about the rename mechanism is therefore closed. The memory entry still goes to this repo's memory directory.

### Custom title bar (no native chrome)

The user: the Windows build must not have "an ugly looking topbar with native ui"; the app "needs to have a clean look so integrate that design all over".

- `tauri.conf.json`: `decorations: false`, `shadow: true` on the main window. The window keeps its minimum size and resizability (Windows still provides edge resizing on undecorated windows with shadow on).
- A `TitleBar` React component renders a 36 px strip across the full window width above the rail and main pane **(my call)**: the Unpack icon and name at the left in the muted text token, a `data-tauri-drag-region` drag area, and three controls at the right (minimize, maximize/restore, close) drawn with lucide icons at 14 px, 46 px wide hit targets, hover background from the surface token, close hover in the danger token. Double-click on the drag area toggles maximize. Controls call `getCurrentWindow().minimize() / toggleMaximize() / close()`.
- Capabilities add `core:window:allow-minimize`, `core:window:allow-toggle-maximize`, `core:window:allow-close`, `core:window:allow-is-maximized`, `core:window:allow-start-dragging` (already present).
- In the browser mock mode the bar renders without the controls acting (no window), so the layout can still be checked.
- The three-pane grid from the v0.1.0 spec sits below the bar; the detail panel and sheets never cover the bar. Light and dark themes both covered by the existing tokens.

### Headless mode (used to load the user's real tasks)

The app binary accepts a few command-line modes so a session (or the user) can work with the database without the window. The window never opens in these modes; the process prints JSON to stdout and exits. **(my call)** They exist to fulfil the last request below honestly, through the same apply/undo path the UI uses, and they stay as a documented power feature.

- `unpack --probe`: prints the provider readiness JSON (same as the `probe_provider` command).
- `unpack --list-tasks`: prints all tasks (including done) as JSON.
- `unpack --import <proposal.json>`: reads a file in the model's response shape (`{summary, operations[]}`), creates a capture whose `model` is the string given by `--as <label>` (default `manual-import`), stores the input file as the capture's single input, runs `resolve` against the open tasks exactly as a model response would, applies **all** resulting operations, and prints the resulting changes. Everything is undoable from Activity like any other capture.
- Errors print one plain sentence to stderr and exit 1.

### GitHub

- A new repository named **Unpack** under the user's personal GitHub account (gajjarparth09@gmail.com), pushed from `master` after `feat/v1` and the Unpack branch are merged. Private by default **(my call)**; the user can flip it.
- The `gh` CLI on this machine is signed in only as the work account. The user must sign the personal account in (`gh auth login`) before the push; the session asks for that at the right moment and does everything else.
- Future commits in this repo use the personal email (`git config user.email` set locally) **(my call)**. Existing commits are not rewritten.

### Build, install and configure on this PC

- `npm run tauri build`, then run the NSIS installer for the current user. Launch Unpack once.
- Provider: Claude Code, which is signed in on this PC with the user's work Claude Teams account, the account they use with claude.ai. The auto-detect default selects it; the session verifies with `unpack --probe` and a headless Test connection equivalent.
- The API-key provider stays available in Settings but is not configured.

### Load the user's real tasks from the planning workspace

After the install, the session reads the planning workspace (the progress-tracker folder first, then each task folder's latest meeting summaries and progress file) and composes a proposal: one top-level task per piece of work with the category set to the task folder name, sub-tasks for the concrete open items, priorities from the agreed order of work, due dates where the notes give one, a short description, and notes with the evidence. A second proposal then marks the items the notes say are done as complete and sets in-progress where work has started. Both go through `unpack --import` so Activity shows them and every change can be undone. The session reports what it loaded and anything it was unsure about, and does not touch the planning files.
