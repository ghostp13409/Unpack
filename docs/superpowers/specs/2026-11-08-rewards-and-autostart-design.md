# Rewards and autostart: design

Date: 2026-11-08
Status: approved in conversation (behaviour); this document is the written form.

## Why

Finishing a task in Unpack is a non-event. The row quietly disappears. The user gets a pile of large tasks
from their manager and works through them for days or weeks. Finishing one should feel like a win.
They want a quick, smooth, slightly bouncy burst of confetti or balloons with a warm sound,
similar to Microsoft To Do. Finishing a big task, meaning all of its subtasks, should feel bigger still.
A light layer of game mechanics should keep them motivated over weeks.

Separately, they want Unpack to start with Windows and open its window.

A clickable mockup of this exists at
`.superpowers/brainstorm/2304-1794164070/content/celebrate.html` (gitignored, local only). It is
the reference for feel, timing, colours and sound. The user tried it and approved it, choosing corner style **A**.

## Goals

- Tiered celebrations, sized to the win (subtask < task < big task), with combos for quick runs.
- A workday streak, today's count, and a weekly recap, visible without cluttering the UI.
- Settings to turn off sound and animation, and an opt-in "start with Windows".

## Non-goals

- XP, levels, points, badges or achievement collections. These were explicitly rejected as cheesy.
- Real audio files. Sounds are synthesized in the browser with Web Audio.
- A system tray, minimize-to-tray, or starting hidden.
- Celebrating changes the AI or the MCP server makes. Only completions the user makes in the window count.

## Anti-goals (failure even if it "works")

- **Compromising the clean look.** The user explicitly said the clean look matters most. Stats live in one
  small, muted element. Everything more is behind hover. No new rails, banners, toasts or panels.
- Slow or lingering effects. Every celebration is short (task about 1.5s, big task about 3s) and never blocks input.
- Celebrations firing when they shouldn't: on page load, on un-checking, on AI-applied completes, or twice for one action.

## Behaviour

### Celebration tiers

These fire only when the user turns a task from not-done to done in the window. That covers **every** user
path: the list checkbox (`TaskRow`), the `d` keyboard shortcut, the subtask checkboxes in the detail panel
(`TaskDetail`), and choosing "Done" in the detail panel's Status dropdown. All of them go through one shared
store action (for example `completeTask` / `toggleDone` with an optional origin), so the celebration, combo and
parent auto-complete behave the same everywhere *(Claude's call: the builder check raised this)*.

**Un-checking a subtask whose parent is done reopens the parent** (status → todo), so a done parent never hides
open work. Re-completing that subtask then gives the Big task tier again *(Claude's call)*.

**Parent auto-complete is a UI rule only.** When the AI or the MCP server completes the last subtask, the
parent is left as it is, which matches today's behaviour. The backend is unchanged *(Claude's call: fits "only the user's own
completions are rewarded" and keeps AI changes predictable and undoable)*.

| Tier | Trigger | Visual | Sound |
|---|---|---|---|
| Subtask | A task with a `parent_id` is completed, and its parent still has open subtasks. | The checkbox pops (scale bounce). No confetti. | A soft, short tick. |
| Task | A top-level task is completed (with or without subtasks; see below). | The checkbox pops, then a confetti burst (about 70 pieces) from the checkbox. The row fades out as it does today. | A warm two-note chime. |
| Big task | (a) The last open subtask of a parent is completed: the parent is then **automatically marked done** too. Or (b) the user completes a parent that has at least 1 subtask, all already done. | Two wide confetti bursts and about 9 balloons floating up from the bottom of the window with a gentle sway. | A rising arpeggio, then a sustained chord and a sparkle. |

- Completing a parent that still has open subtasks is the **Task** tier. Its subtasks are left as they are
  *(builder's call: no cascade, to match today's behaviour)*.
- In (a), only the big celebration plays. The subtask tick is replaced, not layered.
- Un-checking (done → todo) plays nothing. Re-completing later celebrates again.
- Subtasks nested deeper than one level are not expected (the app shows one level). Treat any task with a `parent_id` as a subtask.

### Combos

- If a completion happens within **10 seconds** of the previous one, the combo count goes up. Otherwise it resets.
- Each combo step raises the sound's pitch (chime: +2 semitones per step, capped at +14; tick: +1 semitone, capped at +6).
- From the second completion in a combo, a small "×N" label in the accent colour rises and fades next to the checkbox.
  It shows for Task and Big task tiers, not for subtasks.

### Origin of the effect

- Confetti starts at the checkbox that was clicked (list row or detail-panel subtask).
- For the Status dropdown, it starts at the dropdown element.
- For the keyboard toggle, it starts at the selected row's checkbox if it is on screen. Otherwise it starts at the centre of the main content area.
- Balloons always rise from the bottom of the window.

### Corner stats (title bar, style A)

- Placed in the title bar, just left of the window buttons: a flame icon with the streak number, then a check icon with "N today".
  Muted text (`--muted`), 12px, tabular numbers. Hovering gives a `--surface-2` background, like other quiet controls.
- When the count changes from a completion, the number does a small bump animation.
- It must not be a drag region (hover has to work), but the space around it must stay draggable.
- **Hover** (and keyboard focus) opens a popover under it (280px wide):
  - Large "N done today" and "N day streak" (the streak number in the flame colour).
  - "This week": total done and how many were big tasks, plus a Mon–Fri bar chart. Today is highlighted in accent, the other days use `--accent-soft`.
  - "Best streak: N workdays. Weekends never break it."
- If there's no streak and nothing done today, show it anyway with zeros (a flame and 0). Never hide it.

### Activity view

- A one-line weekly recap at the top of the Activity view: "This week: 14 tasks finished, 3 of them big ones."
  Use the same muted style as the existing page subtitle. Hide it if the week has 0 completions.

### Definitions (all derived from existing task data, no schema change)

- **Completion date:** the local calendar date of `completed_at`, which is stored as UTC ISO. Convert to local time first.
- **Today's count:** tasks with `status = done` and a completion date of today. Subtasks count as tasks.
- **Big task:** a task with `status = done` that has at least 1 child, all of whose children are done.
- **This week:** Monday to Sunday of the current local week. The chart shows Mon–Fri. Weekend completions still count toward the total.
- **Streak:** the number of consecutive workdays (Mon–Fri) with at least 1 completion, counting back from today.
  Weekends are skipped, neither breaking nor adding to it. Weekend completions do not extend it. If today
  is a workday with no completion yet, the streak is counted back from the previous workday (today doesn't
  break it until the day is over).
- **Best streak:** the longest such run in all history.
- Un-checking a task clears `completed_at` (backend already does this), so it drops out of every count automatically.

### Settings

A new **Rewards** section in Settings with two toggles, plus a **Startup** toggle:

- "Celebration sounds": on by default.
- "Celebration animations": on by default. When off, the checkbox pop still happens but there are no confetti or balloons.
  Also suppressed automatically when the OS `prefers-reduced-motion: reduce` is set.
- "Start Unpack when Windows starts": off by default. Uses `tauri-plugin-autostart`. The toggle reflects the plugin's
  real `isEnabled()` state, not a stored preference. When launched at login, the app opens its window normally.
  The autostart entry must launch with **no extra arguments**, so `headless::maybe_run` and `--mcp` handling are never triggered.

The two reward toggles are stored in the existing settings key-value table (new keys `sounds`, `animations`),
following the `Settings` struct pattern with `#[serde(default)]`, defaulting to true.

## How (decided)

- **Stack stays as is:** React + zustand frontend, Tauri 2 Rust backend. No new frontend dependencies for
  effects. Confetti and balloons are drawn on one full-window `<canvas>` overlay (pointer-events: none),
  and sound is synthesized with Web Audio, all ported from the mockup.
- **Autostart:** `tauri-plugin-autostart` (Rust crate and `@tauri-apps/plugin-autostart`), with its permissions added to `capabilities/default.json`.
- **Stats are pure functions** in `src/store/derived.ts` (or a sibling module), unit-tested with vitest like the existing derived tests.
- **Celebration decision logic** (which tier, combo, whether to auto-complete the parent) is a pure function, unit-tested.
  The rendering and audio are thin side effects triggered from the store's `toggleDone`.
- Sounds are synthesized, using the mockup's note choices as the starting point.

## Left to the builder

- Module layout and names (for example `src/lib/celebrate/{sound,fx,decide}.ts`, a `StatsChip` component).
- Exact animation curves and particle counts, within the mockup's feel and the duration limits above.
- How `toggleDone` passes the origin point (an optional argument is fine).
- Auto-completing the parent in tier (a) uses the existing `patchTask` path. Both writes are optimistic. If either fails, the existing error and reload path handles it.
- Lazy-creating the `AudioContext` on first use. The completion click is a user gesture, so autoplay is fine.
