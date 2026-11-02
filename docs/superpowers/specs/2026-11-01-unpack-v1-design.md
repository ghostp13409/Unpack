# Unpack (v1) — Design

Date: 2026-11-01
Status: approved for build. The user asked for read → plan → brainstorm → build in one go, so every choice below marked **(my call)** was made by the builder, not the user, and can be changed later.

## 1. Intent and why

The user gets a stream of large tasks every day from their manager, through meetings, Teams messages, email, and things they simply know they must do. They work on as many as they can, but prioritizing means some tasks are forgotten. Unpack is a personal desktop to-do list that behaves like an assistant: throw context at it (files, pasted text, a sentence), and it creates and maintains the tasks, so nothing gets lost and it's always clear what to work on next.

Source: `docs/overview.md`.

## 2. Goals, non-goals, anti-goals

**Goals**
- Capture context in any form (drop multiple files, paste text, type a line) and have tasks created from it automatically: title, description, priority, category, due date, sub-tasks, and links to larger pre-existing tasks.
- Mentioned existing tasks get updated (new info, re-prioritized, marked done), not duplicated.
- Give a calm, immediate overview: what's open, what's overdue, what's heavy, and a short "work on this next" list with reasons.
- Look clean, modern, professional. Few things on screen; everything easy to find.
- Run locally as a native Windows app (Tauri 2: Rust + React), with a Haiku model doing the reading.

**Non-goals (v1)**
- No live integrations with Teams, Outlook, or a ticket tracker. Context arrives as files or paste. (Exports from those tools are supported as file formats.)
- No multi-user, sync, or cloud storage. One person, one machine, one SQLite file.
- No time tracking, calendars, reminders/notifications, or recurring tasks.
- No mobile.

**Anti-goals (it fails even if it works)**
- An overwhelming screen: dense tables, ten filters, badges everywhere.
- Silent, wrong changes to tasks by the model. Every AI change must be visible and reversible.
- Having to fill in forms. Capturing must be faster than writing a sticky note.

## 3. Constraints
- Windows 11, Rust 1.98, Node 26, WebView2 present. No `tauri` CLI installed yet; added as a dev dependency.
- No `ANTHROPIC_API_KEY` on this machine and no `ant` CLI; the app must have a Settings screen where the key is entered and stored safely.
- Model: Claude Haiku 5.5 (`claude-haiku-5-5`) as the user asked. Rust has no official SDK, so the backend calls the Messages API over raw HTTPS.
- Network is available (npm, crates.io reachable).

## 4. Product shape

### 4.1 Concepts
- **Task**: title, description (markdown), status (`todo`, `in_progress`, `done`, `dropped`), priority (`urgent`, `high`, `medium`, `low`), category (free text label, e.g. "Platform", "Observability", "Admin"), optional due date, optional parent task (one level: a task with children acts as the "larger task/project"), created/updated/completed timestamps, pinned flag, snoozed-until date.
- **Note**: dated text attached to a task. The model adds notes when context mentions an existing task ("Alex said this is now blocked on legal"); the user can add notes too.
- **Capture**: one ingest run. Holds the raw inputs (each pasted text or file with name and extracted text), when it ran, the model's one-paragraph summary, and the list of changes it proposed and which were applied.
- **Change**: one proposed/applied modification from a capture: `create`, `update`, `complete`, `add_note`. Stores before/after so it can be shown and undone.

### 4.2 Capture flow (the core loop)
1. User presses **Ctrl+N** or the **Capture** button. A sheet opens with one big text area and a drop zone ("Drop meeting transcripts, emails, notes…"). Several files plus text at once are fine.
2. Supported inputs: `.txt`, `.md`, `.vtt` (Teams transcript; timing lines stripped, consecutive lines from one speaker merged), `.eml` (headers From/To/Date/Subject kept, body decoded, HTML stripped, quoted reply chains kept), `.json`, `.csv`, `.docx` (text pulled from `word/document.xml`), `.pdf` (sent to the model as a document block, not parsed locally). Anything else: rejected with a clear message.
3. User presses **Process** (Ctrl+Enter). Backend builds one request to Haiku 5.5 containing: a compact list of all open tasks (id, title, status, priority, category, due, parent, a 1-line description), the list of known categories, today's date, and all inputs. Structured output (JSON schema) forces a response of `{summary, operations[]}`.
4. The **Review** step shows what the model wants to do, grouped: *New tasks* (with sub-tasks nested), *Updates* (field diffs shown as "Priority: medium → high"), *Completed*, *Notes added*. Every item has a checkbox, all pre-checked. Buttons: **Apply** (default) / **Discard**. **(my call)** This is one extra click versus fully automatic, but it keeps "silent wrong changes" from happening and it doubles as the "what did it do" view the user needs to trust the tool. It must be fast: Apply is the default action and Enter triggers it.
5. Applied changes land in the task list and in **Activity**. Each applied change can be undone from Activity.

**Undo semantics (my call).** A change stores only the fields it touched, as `before_json`/`after_json` maps (`{"priority": "medium"}` → `{"priority": "high"}`). Undo of:
- `create`: sets the task's status to `dropped` (soft delete, so notes, children and later changes keep valid references). Dropped tasks are hidden everywhere except Activity.
- `update`: writes the `before` values back for the touched fields only. If the task's current values no longer equal `after` for those fields (the user or a later capture changed them), the UI asks "This task changed since. Overwrite?" before undoing.
- `complete`: restores the previous status and clears `completed_at`, same staleness check.
- `add_note`: deletes the note.
Changes may be undone in any order; an undone change is marked `undone = 1` and cannot be re-applied.

**Source snippets (my call).** A task's "Sources" section is derived: every change row links the task to its capture, so the panel lists each capture's date, input filenames, the operation's `reason`, and its `evidence` (an optional short quote, under 200 characters, that the model copies from the context). No local quoting is needed, so PDFs work too.

Quick add without the model: typing a single short line in the Capture sheet and pressing Enter (not Ctrl+Enter) creates a task directly with defaults (priority medium, no category). **(my call)**

### 4.3 "What should I work on next"
Computed locally, no model call, so it's instant and free. **(my call)** Score per open task:
- priority: urgent 40, high 25, medium 10, low 0
- due: overdue +30 (+2 per day overdue, capped +20), due today +25, due within 3 days +15, within 7 days +8
- in progress +10 (finish what's started)
- age: +1 per 3 days open, capped +10 (stops things from rotting)
- pinned +50
- parent task with all sub-tasks done +15 ("close it out")
- Only leaf tasks appear in Up Next: tasks with no open children. A parent with open children is represented by its children.
- Snoozed tasks (snoozed_until in the future) are excluded from Up Next.

The Focus view shows the top 5 with a one-line reason built from the contributing factors ("Overdue by 2 days · High"). The user can pin a task to force it to the top, or snooze it (hide from Up Next for 1/3/7 days).

### 4.4 Views
- **Focus** (home): Up Next (top 5), then "Overdue", "Due this week", and a slim workload strip (open count by priority, overdue count, done this week).
- **Tasks**: everything open; group by Category (default) or Priority or Due; search box; toggle to show done.
- **Activity**: timeline of captures; each expands to its inputs, summary, and applied changes with Undo.
- **Settings**: API key, model (default `claude-haiku-5-5`, editable), effort (`low`/`medium`/`high`, default `medium`), your name (default "Sam") and your manager's name (default empty) so the model knows whose tasks to extract and whose asks rank high **(my call)**, theme (System/Light/Dark), data folder path (read-only, with "Open folder").
- **Task detail** panel (right side): editable title, status, priority, category, due date, parent, description, sub-tasks checklist (add inline), notes, and the source snippets from captures that touched it.

### 4.5 Keyboard
Ctrl+N capture · Ctrl+K search · Ctrl+, settings · J/K move selection · Enter open · D done · E edit title · Esc close panel/sheet. **(my call)**

## 5. Frontend design brainstorm

Three directions were considered.

**A. Quiet workspace** (think Things / Linear). Three panes: a slim icon rail, a list, and a detail panel. Neutral warm canvas, one accent color, priority shown as a small colored dot rather than loud badges. Information appears progressively: a row shows title, category chip, due; hover reveals the checkbox and priority; details live in the panel.

**B. Board.** Kanban columns by status. Rejected: the user's need is "what next and am I forgetting something", not moving cards between states; boards spread information horizontally and get busy fast.

**C. Daily brief.** A single card-centric page per day with a timeline. Rejected as the whole app (tasks here are multi-day and multi-week), but its best idea, a short "up next with reasons" section, is folded into the Focus view of A.

**Chosen: A, with C's Focus page as home.**

Visual language:
- Typeface Inter (bundled via `@fontsource-variable/inter`, no network fonts), 14px base, scale 12/13/14/16/20/28. Tabular numbers for dates.
- Light: canvas `#F7F6F3`, surface `#FFFFFF`, text `#1C1C1A`, muted `#6B6A66`, border `#E7E5E0`. Dark: canvas `#161615`, surface `#1F1F1D`, text `#EDECE8`, muted `#9C9A94`, border `#2C2C29`. Accent (one): `#4F6BED` light / `#8A9CF5` dark. Follows system theme by default.
- Priority dots: urgent `#D9484B`, high `#E0862C`, medium `#4F6BED`, low `#8E8C86`. Due dates turn red when overdue, orange when today.
- Spacing on an 8px grid; list rows 40px; generous 24px page gutters; max content width 880px for the list so lines stay readable.
- Motion: 150ms ease for panels/sheets, nothing bouncy. Checking a task off fades it out after 600ms so the click feels acknowledged.
- Empty states carry one sentence and the Capture button, never an illustration gallery.
- Layout at the default window (1200×800): rail 56px · list flexible · detail 380px (hidden until a task is selected). Below 900px wide, the detail panel becomes an overlay.

Rough screen:

```
┌──┬──────────────────────────────────────────────┬──────────────────┐
│◎ │  Focus                                 ⌘N    │  Task title      │
│☰ │                                              │  ● High  Platform│
│⟲ │  Up next                                     │  Due Nov 4       │
│⚙ │  ○ Finish the error-tracking SDK integration  Nov 4   │  ───────────     │
│  │     Overdue by 1 day · High                  │  Description…    │
│  │  ○ Reply to Alex re: the pricing proposal   │                  │
│  │     Due today · from email Oct 31            │  Sub-tasks  2/4  │
│  │  ○ …                                         │  ☑ …  ☐ …        │
│  │                                              │  Notes           │
│  │  Overdue (2)   Due this week (5)             │  Oct 31 — "…"    │
│  │  ────────────────────────────────────────    │  Sources         │
│  │  12 open · 3 urgent · 2 overdue · 4 done/wk  │  ▸ Call w/ Alex│
└──┴──────────────────────────────────────────────┴──────────────────┘
```

## 6. Architecture

```
React (Vite, TS)  ──invoke()──▶  Tauri commands (Rust)
  zustand store                    ├─ db.rs        rusqlite (bundled), WAL, migrations
  views/, components/              ├─ ingest/      parsers: vtt, eml, docx, text; pdf passthrough
  styles/ (CSS vars, no Tailwind)  ├─ ai/          Messages API client (reqwest+rustls), prompt, schema, op parsing
                                   ├─ ranking.rs   Up Next scoring
                                   ├─ secrets.rs   API key via Windows Credential Manager (keyring crate)
                                   └─ commands.rs  thin command layer + DTOs (serde)
```

**(my calls)** Plain CSS with variables instead of Tailwind to keep the design exact and the bundle small; `zustand` for state; `lucide-react` for icons; `@tauri-apps/plugin-dialog` for file picking and the Tauri window drag-drop event for dropping files onto the window; no ORM.

### 6.1 Data (SQLite, in the Tauri app data dir, e.g. `%APPDATA%\com.parth.unpack\tasks.db`)
```
tasks(id TEXT PK, title, description, status, priority, category, due_date, parent_id, pinned INT,
      snoozed_until, created_at, updated_at, completed_at)
notes(id PK, task_id FK, body, source_capture_id NULL, created_at)
captures(id PK, created_at, summary, model, input_tokens, output_tokens, status)   -- status: proposed|applied|discarded|failed
capture_inputs(id PK, capture_id FK, kind, filename, extracted_text, char_count)
changes(id PK, capture_id FK, task_id, kind, before_json, after_json, reason, evidence, applied INT, undone INT, created_at)
settings(key PK, value)
```
IDs are UUIDv4 strings; timestamps ISO-8601 UTC; dates `YYYY-MM-DD`.

### 6.2 Model call
- `POST https://api.anthropic.com/v1/messages`, headers `x-api-key`, `anthropic-version: 2026-10-21`, `content-type: application/json`.
- Body: `model` (setting, default `claude-haiku-5-5`), `max_tokens: 16000`, `output_config: {effort: <setting>, format: {type: "json_schema", schema: <ops schema>}}`, `system` (stable instructions, with `cache_control: {type: "ephemeral"}`), `messages: [{role: "user", content: [<text block: date + open tasks + categories>, <text/document blocks per input>, <text: "Produce the operations.">]}]`. No `temperature`/`top_p`/`top_k`, no prefill, no `thinking` field (adaptive by default).
- Read `stop_reason` first: `refusal` → show the category and let the user edit inputs; `max_tokens` → tell the user the inputs were too large for one pass; otherwise take the first content block with `type == "text"` and parse JSON.
- Timeout 120s, 2 retries on 429/5xx/network with backoff. Errors surface as a plain sentence in the Capture sheet.
- Size guard: if total extracted text exceeds ~350k characters, refuse with "split this into two captures" before calling.

### 6.3 Operations schema (what the model returns)
```json
{
  "summary": "one paragraph of what this context was about",
  "operations": [
    {"op": "create", "temp_id": "t1", "title": "...", "description": "...", "priority": "high",
     "category": "Platform", "due_date": "2026-11-05", "parent_id": "<existing id> | <temp_id> | null",
     "reason": "why this is a task and why this priority", "evidence": "short quote from the context or null"},
    {"op": "update", "task_id": "<existing id>", "title": null, "description": null, "priority": "urgent",
     "category": null, "due_date": null, "parent_id": null, "reason": "..."},
    {"op": "complete", "task_id": "<existing id>", "reason": "..."},
    {"op": "add_note", "task_id": "<existing id>", "body": "...", "reason": "..."}
  ]
}
```
In practice this is one flat object type with every field present and nullable (structured outputs need `additionalProperties: false` and `required`); which fields matter depends on `op`. Sub-tasks are `create` ops whose `parent_id` is another op's `temp_id`. The backend resolves temp ids, validates every referenced task id exists, drops invalid ops with a visible warning, and never lets the model delete anything.

### 6.4 System prompt (essence; final wording is the builder's)
The system prompt is static (cacheable). The user's name and manager's name from Settings go into the first user-message block alongside the date and open tasks. Essence: You maintain one person's task list. From the given context, extract concrete tasks the user (named in the message) must do, not tasks for other people. Prefer updating an existing task over creating a near-duplicate; match by meaning, not exact words. Create sub-tasks when a task has clearly separable steps mentioned in the context. Set priority from urgency words, deadlines, who asked (the manager's asks default to high), and blocking relationships. Use an existing category when one fits; otherwise create a short new one. Dates: resolve relative dates against today's date given. Titles: imperative, under 80 characters. Give a one-line reason per operation. If the context contains no actionable items, return an empty operations list and say so in the summary.

### 6.5 Secrets
API key stored with the `keyring` crate under service `Unpack`, account `anthropic_api_key`. Never written to the DB or logs. Settings screen shows only "Key saved (…XXXX)" and a Replace button, plus a **Test connection** button that sends a tiny request.

## 7. What's left to the builder
- Exact prompt wording and the few-shot examples, if any.
- Component breakdown, store shape, CSS details within the visual language above.
- Migration mechanism (a simple `schema_version` in `settings` is fine).
- How the Review step renders field diffs.
- Test strategy: Rust unit tests for parsers, ranking, op validation/resolution, and the DB layer; the AI client tested against a fake HTTP server; frontend has a light smoke test (Vitest) for the store's derived views. No end-to-end UI automation in v1.

## 8. Open items deliberately deferred
- Reading Teams/Outlook directly.
- Reminders/notifications.
- A "daily brief" generated by the model (the Focus view is deterministic in v1).
- Packaging an installer (`tauri build` produces one; polishing the icon and signing are later).
