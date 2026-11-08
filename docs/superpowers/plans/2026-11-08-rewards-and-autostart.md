# Rewards and Autostart Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reward task completion with tiered confetti/balloon celebrations, synthesized sounds and combos, show a workday streak and stats in a quiet title-bar chip, and add an opt-in "start with Windows".

**Architecture:** Two pure, unit-tested modules decide everything. `stats.ts` derives today, streak, best streak and week numbers from `completed_at`, and `celebrate/decide.ts` picks the tier and the parent auto-complete or reopen. A single store action, `toggleDone(id, origin?)`, is the only user completion path. It writes optimistically, then calls a thin side-effect layer (`celebrate/fx.ts` canvas plus `celebrate/sound.ts` Web Audio) that is ported from the approved mockup. Autostart uses `tauri-plugin-autostart`.

**Tech Stack:** React 19, zustand 5, vitest, Tauri 2 (Rust, rusqlite), Web Audio, Canvas 2D.

**Spec:** `docs/superpowers/specs/2026-11-08-rewards-and-autostart-design.md`. The mockup is the reference for feel: `.superpowers/brainstorm/2304-1794164070/content/celebrate.html`. Port its sound and particle code. It is gitignored, so read it locally.

## Global Constraints

- No new frontend deps for effects. The only new deps are `tauri-plugin-autostart` (crate) and `@tauri-apps/plugin-autostart` (npm).
- Clean look first: the only new always-visible UI is the title-bar chip (12px, `--muted`, tabular numbers). Everything else is behind hover or in Settings.
- Durations: a task celebration is about 1.5s, a big task about 3s. Never block input (`pointer-events: none`).
- Combo window is 10s. Chime is +2 semitones per step, capped at +14. Tick is +1 semitone per step, capped at +6. "×N" shows from the 2nd completion in a combo, for the task and big tiers only.
- Celebrate only user transitions to done made in the window. Never on load, on un-check, or on AI or MCP changes.
- `prefers-reduced-motion: reduce` suppresses confetti and balloons (the checkbox pop stays).
- Settings `sounds` and `animations` both default to `true`. Autostart is off by default, reads its state from the plugin's `isEnabled()`, and registers with **no args**.
- Streak counts Mon–Fri only. Weekends neither break it nor extend it. Today without a completion doesn't break it.
- All dates are **local** calendar dates derived from UTC `completed_at`.

## Review Focus

1. **Quick double-click on a checkbox:** the second click must not play a second celebration or leave the parent in a half-done state. `toggleDone` ignores a task already in the target state, and decide only celebrates todo → done. Test in Task 3.
2. **Un-checking the last subtask of an auto-completed parent:** the parent reopens, and re-checking celebrates big again. Test in Task 3.
3. **Completions near midnight / UTC offset:** a task completed at 23:30 local on Monday that is stored as Tuesday UTC must count as Monday. Test in Task 1 (use `new Date(y,m,d,h)` to build the ISO string, so it works in any timezone).
4. **Monday morning with nothing done yet:** the streak shows Friday's run, not 0. Test in Task 1.
5. **Status dropdown set to "In progress" on a done task:** this is an un-check, so no celebration, and the parent reopens if it was done. Test in Task 3.

---

### Task 1: Stats derivations

**Files:**
- Create: `src/store/stats.ts`
- Test: `src/store/stats.test.ts`

**Interfaces:**
- Produces:
  - `localDay(iso: string): string`: the local `YYYY-MM-DD` of a timestamp.
  - `isBigTask(t: Task, all: Task[]): boolean`: `status === "done"`, at least 1 child, and all children done.
  - `computeStats(tasks: Task[], now?: Date): Stats` where
    `interface Stats { today: number; streak: number; best: number; week: { days: number[] /* Mon..Fri counts */; total: number /* Mon..Sun */; big: number /* big tasks completed this week */ }; todayIndex: number /* 0..4 Mon..Fri, -1 on weekend */ }`
  - Only tasks with `status === "done"` and a non-null `completed_at` count.

- [ ] **Step 1: Write failing tests** in `src/store/stats.test.ts`. Reuse the `task()` factory pattern from `derived.test.ts`. Use a helper `at(y, mon, d, h = 12) => new Date(y, mon - 1, d, h).toISOString()`. Fix `now = new Date(2026, 10, 11, 15)` (a Wednesday). Tests:
  - `today counts tasks done today including subtasks`: 2 done today (one a subtask) and 1 done yesterday → `today === 2`.
  - `uses local day not UTC`: done at `at(2026,11,9,23,30)` (pass minutes) → counts toward Monday (`week.days[0] === 1`).
  - `streak counts consecutive workdays back from today`: done Mon, Tue and Wed → `streak === 3`.
  - `weekend never breaks streak`: done on Thu 5 Nov, Fri 6, Mon 9, Tue 10, Wed 11 → `streak === 5`.
  - `today without completion does not break streak`: done Mon and Tue, nothing Wed → `streak === 2`.
  - `gap on a workday breaks streak`: done Mon and Wed (not Tue) → `streak === 1`.
  - `weekend completion does not extend streak`: done only on Sat 7 and Sun 8 → `streak === 0`.
  - `best streak is longest run in history`: a 4-workday run in October plus the current 2 → `best === 4`. If the current run is longer, `best === streak`.
  - `week totals include weekend, chart is Mon-Fri`: done on Mon 9 and Sun 15 → `week.total === 2`, `week.days === [1,0,0,0,0]`.
  - `big task counted when all children done`: a parent done this week with 2 done children → `week.big === 1`. A parent with 0 children → not big.
  - `ignores not-done tasks with stale completed_at`: `status: "todo", completed_at: <today>` → `today === 0`.
  - `todayIndex is -1 on weekend`: `now` = Sat → `todayIndex === -1`, and the streak equals Friday's run.
- [ ] **Step 2: Run** `npx vitest run src/store/stats.test.ts`. Expect FAIL (the module is missing).
- [ ] **Step 3: Implement `src/store/stats.ts`.** Build a `Set` of local days that have ≥1 completion. Streak: start at today if it's a workday with a completion, otherwise at the previous workday. Walk back over workdays (skipping Sat/Sun) while the day is in the set. Best: sort the distinct workday dates ascending and count runs, where "next workday" of Fri is Mon. Week starts on Monday.
- [ ] **Step 4: Run** `npx vitest run src/store/stats.test.ts`. Expect PASS.
- [ ] **Step 5: Commit** `feat: derive completion stats and workday streak`

### Task 2: Celebration decision logic

**Files:**
- Create: `src/lib/celebrate/decide.ts`
- Test: `src/lib/celebrate/decide.test.ts`

**Interfaces:**
- Consumes: nothing from Task 1. Decide works on the pre-toggle task list.
- Produces:
  - `type Tier = "subtask" | "task" | "big"`
  - `interface Decision { celebrate: Tier | null; completeParentId: string | null; reopenParentId: string | null }`
  - `decide(tasks: Task[], id: string, next: Status): Decision`. `tasks` is the state **before** the change.
  - `nextCombo(prev: { at: number; count: number } | null, now: number): { at: number; count: number }`. `count` is 0 for the first completion and +1 when `now - prev.at < 10_000`.
  - `COMBO_WINDOW_MS = 10_000`

- [ ] **Step 1: Write failing tests:**
  - `top-level task todo→done is task tier`: `{celebrate:"task", completeParentId:null, reopenParentId:null}`.
  - `subtask done while siblings open is subtask tier`.
  - `last open subtask completes parent with big tier`: parent P is todo, children A (done) and B (todo). `decide(tasks,"B","done")` → `{celebrate:"big", completeParentId:"P", reopenParentId:null}`.
  - `last subtask under an already-done parent is subtask tier and completes nothing`.
  - `completing parent whose children are all done is big`.
  - `completing parent with open children is task tier`.
  - `un-checking anything never celebrates`: done → todo gives `celebrate:null`.
  - `un-checking a subtask reopens a done parent`: P is done, A is done. `decide(tasks,"A","todo")` → `reopenParentId:"P"`.
  - `done to in_progress counts as un-check`: same as above with `next:"in_progress"`.
  - `no-op when already in target state`: A is already done, `next:"done"` → all nulls.
  - `dropped siblings are ignored when checking all-done`: a sibling with `status:"dropped"` doesn't block the big tier.
  - `nextCombo`: null → count 0; within 9_999ms → count+1; at exactly 10_000ms → 0.
- [ ] **Step 2: Run** `npx vitest run src/lib/celebrate/decide.test.ts`. Expect FAIL.
- [ ] **Step 3: Implement `decide.ts`.** "Done" means `status === "done"`. Children are the tasks with `parent_id === parent.id && status !== "dropped"`.
- [ ] **Step 4: Run.** Expect PASS.
- [ ] **Step 5: Commit** `feat: decide celebration tier and parent auto-complete`

### Task 3: Effects layer and store integration

**Files:**
- Create: `src/lib/celebrate/sound.ts`, `src/lib/celebrate/fx.ts`, `src/lib/celebrate/index.ts`
- Modify: `src/store/useStore.ts` (`toggleDone` at lines 31 and 74–80), `src/components/TaskRow.tsx:45`, `src/components/Checkbox.tsx`, `src/hooks/useShortcuts.ts:83-90`, `src/views/TaskDetail.tsx:89-99` (status select) and `:191-195` (sub-task checkbox), `src/styles/components.css` (checkbox pop, `.combo-pop`)
- Test: `src/store/toggleDone.test.ts`

**Interfaces:**
- Consumes: `decide`, `nextCombo`, `Tier` (Task 2).
- Produces:
  - `type Origin = { x: number; y: number }`
  - `celebrate(opts: { tier: Tier; combo: number; origin: Origin | null; sound: boolean; motion: boolean }): void` in `celebrate/index.ts`. It plays `playTick | playChime | playBig` and `burst | bigCelebrate`, and shows the "×N" label when `combo > 0 && tier !== "subtask" && motion`. A `null` origin falls back to the centre of `.main`.
  - `sound.ts`: `playTick(combo)`, `playChime(combo)`, `playBig()`. Lazy `AudioContext`. Port the note values from the mockup verbatim.
  - `fx.ts`: `burst(origin, count, power)`, `bigCelebrate()`. A lazily created fixed full-window `<canvas>` appended to `body` (`pointer-events:none; z-index:50`), DPR-aware, removed or cleared when idle. Port from the mockup: about 70 pieces for a task, 2×90 plus 9 balloons for big. Colours: `#4f6bed #8a9cf5 #ef7a2a #f2c14e #3b9a62 #e5697b #b18cf0`.
  - Store: `toggleDone(id: string, origin?: Origin, next?: Status): Promise<void>`. `next` defaults to flipping done/todo. TaskDetail's select passes its value. Module-level `combo` state lives in the store file.
  - `Checkbox` `onChange` signature becomes `(origin: Origin) => void`, where origin is the centre of the button's `getBoundingClientRect()`.

- [ ] **Step 1: Write failing store tests** in `src/store/toggleDone.test.ts`. Use `vi.mock("../lib/api", ...)` so `updateTask` returns the patched task and `upNext` returns `[]`. Use `vi.mock("../lib/celebrate", () => ({ celebrate: vi.fn() }))`. Seed `useStore.setState({ tasks, settings: { settings: { sounds: true, animations: true, ... } } })`.
  - `completing last subtask marks parent done and celebrates big once`: after `await toggleDone("B")`, both B and P have `status:"done"` in the store, `updateTask` is called for B and for P, and `celebrate` is called once with `tier:"big"`.
  - `un-checking subtask reopens parent and does not celebrate`: P and A are done. `toggleDone("A")` → both todo, `celebrate` not called.
  - `double toggle to done celebrates once`: `toggleDone("T", undefined, "done")` twice → `celebrate` called once.
  - `status select done→in_progress reopens parent`: `toggleDone("A", undefined, "in_progress")` → P is todo, no celebration.
  - `respects settings flags`: with `sounds:false, animations:false`, `celebrate` is called with `sound:false, motion:false`. Motion is also false when `matchMedia("(prefers-reduced-motion: reduce)").matches`. Stub `window.matchMedia`.
  - `combo increments within 10s`: use `vi.useFakeTimers()` and `vi.setSystemTime`. Two task completions 3s apart → the second call has `combo:1`.
- [ ] **Step 2: Run** `npx vitest run src/store/toggleDone.test.ts`. Expect FAIL.
- [ ] **Step 3: Implement `sound.ts`, `fx.ts`, `index.ts`** by porting from the mockup. Settings fields `sounds` and `animations` come from Task 4. Until Task 4 lands, read them as `settings?.settings.sounds ?? true`.
- [ ] **Step 4: Rewrite `toggleDone`:**
  1. Get `next` and `decide(get().tasks, id, next)`. If the task is already in `next`, return.
  2. Apply the task change (and the parent change, if any) to the store optimistically.
  3. If `decision.celebrate`, update the combo and call `celebrate` **before** awaiting the network.
  4. `await patchTask(id, {status: next})`, then `patchTask(parentId, {status: "done" | "todo"})` when the decision says so.
- [ ] **Step 5: Route every user path through it:**
  - `TaskRow`: `onChange={(o) => toggleDone(task.id, o)}`.
  - Shortcut: origin from `document.querySelector(`[data-task-id="${id}"] .row-check`)` if present. Add `data-task-id` to the row root in `TaskRow`.
  - TaskDetail sub-task `Checkbox`: `toggleDone(c.id, o)` then `refresh()`.
  - TaskDetail status select: if the status changes to or from `done`, call `toggleDone(task.id, originOf(e.currentTarget), value)` and refresh. Otherwise keep `save`.
- [ ] **Step 6: Add CSS.** `.row-check.checked` gets `animation: check-pop .45s cubic-bezier(.3,1.7,.5,1)`. Add `.combo-pop` (fixed, 12px bold `--accent`, rise 28px and fade over .9s). Both go under `@media (prefers-reduced-motion: no-preference)`.
- [ ] **Step 7: Run** `npm test`. Expect all tests to pass. Then run `npm run build`. Expect no TS errors.
- [ ] **Step 8: Commit** `feat: tiered completion celebrations with combos`

### Task 4: Reward settings (sounds, animations)

**Files:**
- Modify: `src-tauri/src/settings.rs` (struct, `Default`, `KEYS`, `load`, `save`, tests), `src/lib/types.ts:137-146`, `src/lib/mockApi.ts:68`, `src/views/SettingsView.tsx` (new section after Appearance)

**Interfaces:**
- Produces: `Settings.sounds: bool` and `Settings.animations: bool` (Rust, `#[serde(default = "yes")]` where `fn yes() -> bool { true }`), stored as `"true"`/`"false"` under the keys `sounds` and `animations`. A missing key loads as `true`. TS: `sounds: boolean; animations: boolean`.

- [ ] **Step 1: Write failing Rust tests** in `settings.rs`:
  - `defaults_when_nothing_saved` asserts `s.sounds && s.animations`.
  - `save_then_load_roundtrips` with `sounds: false, animations: false` roundtrips.
  - `missing_reward_keys_default_true_after_older_save`: write only `model`, then `load` → both true.
  - Update the existing struct literal at line 131 with the new fields.
- [ ] **Step 2: Run** `cargo test --manifest-path src-tauri/Cargo.toml settings`. Expect FAIL.
- [ ] **Step 3: Implement.** `load` parses `get(..).map(|v| v != "false").unwrap_or(true)`. `save` writes both. `KEYS` stays for the string fields. Write the bools separately to keep types simple.
- [ ] **Step 4: Run.** Expect PASS. Update the TS type and the mockApi defaults (`sounds: true, animations: true`).
- [ ] **Step 5: Add the Settings UI.** Add a `<section className="settings-section"><h3>Rewards</h3>` with two checkbox rows in the existing `settings-grid` pattern:
  - "Celebration sounds", hint: "A short chime when you finish something."
  - "Celebration animations", hint: "Confetti and balloons. Also off when Windows' reduce-motion setting is on."
  
  Each change calls `save({ ...draft, sounds: v })`.
- [ ] **Step 6: Remove the `?? true` fallbacks** from Task 3. Run `npm test && npm run build`. Expect PASS.
- [ ] **Step 7: Commit** `feat: settings to turn celebration sound and animation off`

### Task 5: Stats chip in the title bar and Activity recap

**Files:**
- Create: `src/components/StatsChip.tsx`
- Modify: `src/components/TitleBar.tsx` (insert before `.tb-controls`), `src/styles/components.css`, `src/views/ActivityView.tsx:139`

**Interfaces:**
- Consumes: `computeStats(tasks)`, `Stats` (Task 1).
- Produces: `<StatsChip />` reads `useStore(s => s.tasks)` and memoizes `computeStats`. It recomputes on a 60s interval so it rolls over at midnight.

- [ ] **Step 1: Build `StatsChip`.** It matches mockup style A:
  - Chip: `[flame svg in #ef7a2a] {streak}` then `[lucide Check 12px] {today} today`.
  - Popover (280px, `--surface`, `--border`, `--shadow`, radius 10px):
    - Big `{today}` "done today" and big `{streak}` "day streak" (in the flame colour).
    - "This week" with `{total} done · {big} big`.
    - A Mon–Fri bar chart: today bar `--accent`, others `--accent-soft`, min height 3px, max 38px, scaled to the week's max.
    - `Best streak: {best} workdays. Weekends never break it.`
  
  Opens on hover and on `:focus-within` (the chip is a `button` with `aria-label="Stats: {streak} day streak, {today} done today"`). The chip is **not** `data-tauri-drag-region`. When `today` increases, the value gets a `.bump` class for 500ms (a scale 1→1.35→1 keyframe). Add a CSS custom property `--flame: #ef7a2a` in `tokens.css`.
- [ ] **Step 2: Add the Activity recap.** Under the Activity `page-sub`, when `week.total > 0`, render `<div className="page-sub">This week: {total} tasks finished{big ? `, ${big} of them big ones` : ""}.</div>`. Use "1 task" for singular and "1 of them a big one" when `big === 1`.
- [ ] **Step 3: Verify.** Run `npm run build` (expect PASS). Then `npm run dev` and open http://localhost:5173 (mock API). Check:
  - The chip sits left of the window buttons.
  - Hover shows the popover.
  - Ticking a task bumps the count and shows confetti with sound.
  - Ticking the last subtask in the detail panel gives balloons.
  - Dark theme looks right.
  - The title bar is still draggable on either side of the chip (Tauri only, checked in Task 6).
- [ ] **Step 4: Commit** `feat: streak and stats chip in title bar, weekly recap in Activity`

### Task 6: Start with Windows

**Files:**
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs:28-30`, `src-tauri/capabilities/default.json`, `package.json` (via npm), `src/lib/api.ts` (wrappers), `src/lib/mockApi.ts`, `src/views/SettingsView.tsx`

**Interfaces:**
- Produces: in `api.ts`, `autostartEnabled(): Promise<boolean>` and `setAutostart(on: boolean): Promise<void>`. In Tauri these use `@tauri-apps/plugin-autostart` `isEnabled/enable/disable`. Mock: an in-memory boolean.

- [ ] **Step 1: Add deps.** Run `npm i @tauri-apps/plugin-autostart@^2` and add `tauri-plugin-autostart = "2"` under the desktop deps in `Cargo.toml`.
- [ ] **Step 2: Register the plugin** in `lib.rs`: `.plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))`. `None` means no args, so `headless::maybe_run` and `--mcp` are never triggered. Add the permissions `autostart:allow-enable`, `autostart:allow-disable` and `autostart:allow-is-enabled` to `capabilities/default.json`.
- [ ] **Step 3: Add the Settings UI.** Add a `<section className="settings-section"><h3>Startup</h3>` with a checkbox "Start Unpack when Windows starts" and hint "Opens Unpack when you sign in." On mount, read `autostartEnabled()`. On change, call `setAutostart(v)` then re-read. Errors go to `setError`.
- [ ] **Step 4: Verify.** Run `cargo build --manifest-path src-tauri/Cargo.toml`. Expect PASS. Run `npm run tauri dev`, toggle it on, then run `reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. Expect an `Unpack` entry pointing at the exe with no arguments. Toggle it off and the entry is gone. In the same run, check that the title bar still drags around the stats chip and that the celebrations play in the real window.
- [ ] **Step 5: Commit** `feat: option to start Unpack with Windows`

### Task 7: Version bump and docs

- [ ] Bump the version to `0.4.0` in `package.json`, `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`. Add a short "Rewards" and "Start with Windows" note to `README.md`'s feature list. Run `npm test && cargo test --manifest-path src-tauri/Cargo.toml`. Expect all to pass. Commit `chore: v0.4.0`.
