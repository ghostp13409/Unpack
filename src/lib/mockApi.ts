/**
 * In-browser stand-in for the Tauri backend, used only when the app runs
 * outside Tauri (plain `npm run dev` in a browser) so the UI can be exercised
 * without the native shell. Never loaded inside the real app.
 */
import { addDays, todayISO } from "./dates";
import type {
  Capture,
  CaptureSummary,
  Change,
  NewTask,
  Note,
  ProviderStatus,
  Ranked,
  Settings,
  SettingsView,
  Task,
  TaskDetail,
  TaskPatch,
  UndoOutcome,
} from "./types";

let seq = 0;
const id = () => `mock-${++seq}`;
const now = () => new Date().toISOString().replace(/\.\d+Z$/, "Z");
const today = todayISO();

function mk(title: string, over: Partial<Task> = {}): Task {
  return {
    id: id(),
    title,
    description: "",
    status: "todo",
    priority: "medium",
    category: null,
    due_date: null,
    parent_id: null,
    pinned: false,
    snoozed_until: null,
    created_at: now(),
    updated_at: now(),
    completed_at: null,
    ...over,
  };
}

const tracking = mk("Finish the error-tracking SDK integration", {
  priority: "high",
  category: "Observability",
  due_date: addDays(today, -1),
  description: "Wire the Go services to error tracking with release tagging. Jordan wants it before the next deploy.",
  created_at: "2026-10-20T14:00:00Z",
});
const tasks: Task[] = [
  tracking,
  mk("Add release tagging to the Go build", { parent_id: tracking.id, category: "Observability", status: "done", completed_at: now() }),
  mk("Verify alerts reach the on-call channel", { parent_id: tracking.id, category: "Observability" }),
  mk("Reply to Alex about the pricing proposal", { priority: "high", category: "Pricing", due_date: today, created_at: "2026-10-31T09:00:00Z" }),
  mk("Draft the Atlas loader runbook", { category: "Platform", due_date: addDays(today, 3), created_at: "2026-10-28T09:00:00Z" }),
  mk("Review firewall rules with Jordan", { priority: "urgent", category: "Cloud", due_date: addDays(today, 1) }),
  mk("Clean up stale ticket drafts", { priority: "low", category: "Admin", created_at: "2026-10-05T09:00:00Z" }),
  mk("Write the quarterly planning summary", { category: "Platform", due_date: addDays(today, 6), status: "in_progress" }),
  mk("Book 1:1 with Taylor", { status: "done", completed_at: addDays(today, -2) + "T10:00:00Z", category: "Admin" }),
];
const notes: Note[] = [
  { id: id(), task_id: tracking.id, body: "Jordan said staging DSN is ready; prod DSN comes after the security review.", source_capture_id: "cap-1", created_at: "2026-10-30T16:00:00Z" },
];
let settings: Settings = { model: "claude-haiku-5-5", effort: "medium", user_name: "Sam", manager_name: "Alex Morgan", theme: "system", provider: "", claude_code_path: "", sounds: true, animations: true };
let hasKey = false;
let autostart = false;
/** Browser stand-in: Claude Code counts as installed and signed in unless the path override says "missing". */
function mockStatus(): ProviderStatus {
  const ccReady = settings.claude_code_path.trim() !== "missing";
  const provider = settings.provider || (ccReady ? "claude_code" : "api_key");
  if (provider === "claude_code") {
    return ccReady
      ? { provider, ready: true, detail: "", account: "Signed in as sam@example.com (Example Org, team plan)", checked_at: now() }
      : { provider, ready: false, detail: "Claude Code is not installed, or Unpack cannot find it. Set the path in Settings.", account: "", checked_at: now() };
  }
  return hasKey
    ? { provider, ready: true, detail: "Key saved", account: "", checked_at: now() }
    : { provider, ready: false, detail: "Add your Anthropic API key in Settings first.", account: "", checked_at: now() };
}
function notReady(): string | null {
  const st = mockStatus();
  return st.ready ? null : st.detail;
}
const captures: CaptureSummary[] = [
  {
    capture: { id: "cap-1", created_at: "2026-10-30T16:00:00Z", summary: "Call with Jordan about the error-tracking rollout and firewall rules.", model: "claude-haiku-5-5", input_tokens: 4120, output_tokens: 380, status: "applied", proposal: null },
    inputs: [{ id: id(), capture_id: "cap-1", kind: "file", filename: "Call with Jordan.vtt", extracted_text: "", char_count: 12840 }],
    changes: [
      { id: "ch-1", capture_id: "cap-1", task_id: tracking.id, kind: "update", before: { priority: "medium" }, after: { priority: "high" }, reason: "Jordan wants it before the next deploy", evidence: "we need this in before the deploy on Thursday", applied: true, undone: false, created_at: "2026-10-30T16:00:00Z" },
      { id: "ch-2", capture_id: "cap-1", task_id: tracking.id, kind: "add_note", before: {}, after: { note_id: notes[0].id, body: notes[0].body }, reason: "new information", evidence: null, applied: true, undone: false, created_at: "2026-10-30T16:00:00Z" },
    ],
  },
];

const wait = (ms = 60) => new Promise((r) => setTimeout(r, ms));

function score(t: Task): { score: number; reasons: string[] } {
  let s = 0;
  const reasons: string[] = [];
  if (t.pinned) {
    s += 50;
    reasons.push("Pinned");
  }
  const p = { urgent: 40, high: 25, medium: 10, low: 0 }[t.priority];
  s += p;
  reasons.push(t.priority[0].toUpperCase() + t.priority.slice(1));
  if (t.due_date) {
    const d = Math.round((Date.parse(t.due_date) - Date.parse(today)) / 86400000);
    if (d < 0) {
      s += 30 + Math.min(20, -d * 2);
      reasons.push(`Overdue by ${-d} day${-d === 1 ? "" : "s"}`);
    } else if (d === 0) {
      s += 25;
      reasons.push("Due today");
    } else if (d <= 3) {
      s += 15;
      reasons.push(`Due in ${d} day${d === 1 ? "" : "s"}`);
    } else if (d <= 7) {
      s += 8;
      reasons.push(`Due in ${d} days`);
    }
  }
  if (t.status === "in_progress") {
    s += 10;
    reasons.push("In progress");
  }
  return { score: s, reasons };
}

export const mockApi = {
  async listTasks(includeDone: boolean): Promise<Task[]> {
    await wait();
    return tasks.filter((t) => t.status !== "dropped" && (includeDone || t.status === "todo" || t.status === "in_progress"));
  },
  async getTaskDetail(tid: string): Promise<TaskDetail> {
    await wait();
    const task = tasks.find((t) => t.id === tid);
    if (!task) throw "Task not found";
    const sources = captures.flatMap((c) =>
      c.changes.filter((ch) => ch.task_id === tid).map((ch) => ({
        capture_id: c.capture.id,
        captured_at: c.capture.created_at,
        filenames: c.inputs.map((i) => i.filename ?? "Pasted text"),
        kind: ch.kind,
        reason: ch.reason,
        evidence: ch.evidence,
        undone: ch.undone,
      })),
    );
    return { task, notes: notes.filter((n) => n.task_id === tid), children: tasks.filter((t) => t.parent_id === tid && t.status !== "dropped"), sources };
  },
  async createTask(n: NewTask): Promise<Task> {
    await wait();
    const t = mk(n.title, { description: n.description ?? "", priority: n.priority ?? "medium", category: n.category ?? null, due_date: n.due_date ?? null, parent_id: n.parent_id ?? null });
    tasks.push(t);
    return t;
  },
  async updateTask(tid: string, patch: TaskPatch): Promise<Task> {
    await wait();
    const t = tasks.find((x) => x.id === tid);
    if (!t) throw "Task not found";
    Object.assign(t, patch, { updated_at: now() });
    if (patch.status) t.completed_at = patch.status === "done" ? now() : null;
    return { ...t };
  },
  async addNote(task_id: string, body: string): Promise<Note> {
    const n = { id: id(), task_id, body, source_capture_id: null, created_at: now() };
    notes.push(n);
    return n;
  },
  async deleteNote(nid: string) {
    const i = notes.findIndex((n) => n.id === nid);
    if (i >= 0) notes.splice(i, 1);
  },
  async listCategories() {
    return [...new Set(tasks.filter((t) => t.status !== "dropped").map((t) => t.category).filter(Boolean) as string[])].sort();
  },
  async upNext(limit: number): Promise<Ranked[]> {
    const open = tasks.filter((t) => t.status === "todo" || t.status === "in_progress");
    const leaf = open.filter((t) => !open.some((c) => c.parent_id === t.id));
    return leaf
      .map((t) => ({ task_id: t.id, ...score(t) }))
      .sort((a, b) => b.score - a.score)
      .slice(0, limit);
  },
  async startCapture(text: string | null, paths: string[]): Promise<Capture> {
    await wait(900);
    const blocked = notReady();
    if (blocked) throw blocked;
    const cap: Capture = {
      id: id(),
      created_at: now(),
      model: settings.model,
      input_tokens: 2000,
      output_tokens: 200,
      status: "proposed",
      summary: `Mock proposal from ${paths.length} file(s) and ${text ? "pasted text" : "no text"}.`,
      proposal: {
        summary: "Alex asked for the pricing proposal by Friday and wants the error-tracking work wrapped up; the firewall review moved to next week.",
        ops: [
          { index: 0, op: { type: "create", temp_id: "t1", new: { title: "Send the pricing proposal to Alex", description: "Revised proposal based on volumes.", priority: "high", category: "Pricing", due_date: addDays(today, 4), parent_id: null }, parent_temp: null }, reason: "Manager asked, due Friday", evidence: "can you get me the pricing by Friday" },
          { index: 1, op: { type: "create", temp_id: "t2", new: { title: "Pull volume numbers from the vendor", description: "", priority: "medium", category: "Pricing", due_date: null, parent_id: null }, parent_temp: "t1" }, reason: "Separable step", evidence: null },
          { index: 2, op: { type: "update", task_id: tasks[5].id, patch: { due_date: addDays(today, 8), priority: "high" } }, reason: "Moved to next week", evidence: "let's push the firewall review to next week" },
          { index: 3, op: { type: "complete", task_id: tasks[2].id }, reason: "Confirmed in the call", evidence: "alerts are showing up in on-call now" },
        ],
        warnings: [],
      },
    };
    captures.unshift({ capture: cap, inputs: paths.map((p) => ({ id: id(), capture_id: cap.id, kind: "file" as const, filename: p.split(/[\\/]/).pop() ?? p, extracted_text: "", char_count: 1000 })), changes: [] });
    return cap;
  },
  async applyCapture(captureId: string, selected: number[]): Promise<Change[]> {
    await wait();
    const entry = captures.find((c) => c.capture.id === captureId);
    if (!entry?.capture.proposal) throw "Capture not found";
    const temp = new Map<string, string>();
    for (const op of entry.capture.proposal.ops.filter((o) => selected.includes(o.index))) {
      let ch: Change | null = null;
      if (op.op.type === "create") {
        const t = await mockApi.createTask({ ...op.op.new, parent_id: op.op.parent_temp ? temp.get(op.op.parent_temp) ?? null : op.op.new.parent_id });
        if (op.op.temp_id) temp.set(op.op.temp_id, t.id);
        ch = { id: id(), capture_id: captureId, task_id: t.id, kind: "create", before: {}, after: { title: t.title }, reason: op.reason, evidence: op.evidence, applied: true, undone: false, created_at: now() };
      } else if (op.op.type === "update") {
        const t = tasks.find((x) => x.id === (op.op as { task_id: string }).task_id)!;
        const before: Record<string, unknown> = {};
        for (const k of Object.keys(op.op.patch)) before[k] = t[k as keyof Task];
        await mockApi.updateTask(t.id, op.op.patch);
        ch = { id: id(), capture_id: captureId, task_id: t.id, kind: "update", before, after: { ...op.op.patch }, reason: op.reason, evidence: op.evidence, applied: true, undone: false, created_at: now() };
      } else if (op.op.type === "complete") {
        await mockApi.updateTask(op.op.task_id, { status: "done" });
        ch = { id: id(), capture_id: captureId, task_id: op.op.task_id, kind: "complete", before: { status: "todo" }, after: { status: "done" }, reason: op.reason, evidence: op.evidence, applied: true, undone: false, created_at: now() };
      } else {
        const n = await mockApi.addNote(op.op.task_id, op.op.body);
        ch = { id: id(), capture_id: captureId, task_id: op.op.task_id, kind: "add_note", before: {}, after: { note_id: n.id, body: n.body }, reason: op.reason, evidence: op.evidence, applied: true, undone: false, created_at: now() };
      }
      entry.changes.push(ch);
    }
    entry.capture.status = entry.changes.length ? "applied" : "discarded";
    return entry.changes;
  },
  async discardCapture(captureId: string) {
    const e = captures.find((c) => c.capture.id === captureId);
    if (e) e.capture.status = "discarded";
  },
  async listCaptures(limit: number): Promise<CaptureSummary[]> {
    await wait();
    return captures.slice(0, limit);
  },
  async undoChange(changeId: string, force: boolean): Promise<UndoOutcome> {
    const ch = captures.flatMap((c) => c.changes).find((c) => c.id === changeId);
    if (!ch || !ch.task_id) throw "change not found";
    const t = tasks.find((x) => x.id === ch.task_id)!;
    if (ch.kind === "update" && !force) {
      const changed = Object.keys(ch.after).filter((k) => t[k as keyof Task] !== ch.after[k]);
      if (changed.length) return { type: "needs_confirm", changed_fields: changed };
    }
    if (ch.kind === "create") t.status = "dropped";
    else if (ch.kind === "update") Object.assign(t, ch.before);
    else if (ch.kind === "complete") {
      t.status = "todo";
      t.completed_at = null;
    } else await mockApi.deleteNote(String(ch.after.note_id));
    ch.undone = true;
    return { type: "done" };
  },
  async getSettings(): Promise<SettingsView> {
    await wait();
    return { settings, has_key: hasKey, key_hint: hasKey ? "\u2026mock" : null, data_dir: "C:\\Users\\you\\AppData\\Roaming\\com.parth.unpack", provider_status: mockStatus() };
  },
  async probeProvider(): Promise<ProviderStatus> {
    await wait(300);
    const st = mockStatus();
    settings = { ...settings, provider: st.provider };
    return st;
  },
  async saveSettings(s: Settings) {
    settings = { ...s };
  },
  async setApiKey(key: string) {
    if (!key.trim()) throw "The API key is empty.";
    hasKey = true;
  },
  async clearApiKey() {
    hasKey = false;
  },
  async testConnection(): Promise<string> {
    await wait(600);
    const blocked = notReady();
    if (blocked) throw blocked;
    return "claude-haiku-5-5";
  },
  async openDataFolder() {
    /* no-op in the browser */
  },
  async autostartEnabled() {
    return autostart;
  },
  async setAutostart(on: boolean) {
    autostart = on;
  },
};
