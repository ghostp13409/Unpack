import { create } from "zustand";
import * as api from "../lib/api";
import { celebrate, type Origin } from "../lib/celebrate";
import { decide, nextCombo } from "../lib/celebrate/decide";
import type { CaptureSummary, Ranked, SettingsView, Status, Task, TaskPatch } from "../lib/types";
import type { GroupBy } from "./derived";

/** Last celebrated completion, for combos. Session-only, never persisted. */
let combo: { at: number; count: number } | null = null;

/** How long a just-completed row stays in an open-only list so its tick can be seen. */
export const LINGER_MS = 1000;

function prefersReducedMotion(): boolean {
  return typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches === true;
}

export type View = "focus" | "tasks" | "activity" | "settings";

interface StoreState {
  tasks: Task[];
  loaded: boolean;
  showDone: boolean;
  /** Ids just completed from the UI; open-only lists keep showing them for LINGER_MS. */
  lingering: string[];
  selectedId: string | null;
  /** Keyboard cursor (J/K) over the visible rows; independent of the open detail. */
  cursorId: string | null;
  view: View;
  groupBy: GroupBy;
  query: string;
  upNext: Ranked[];
  captureOpen: boolean;
  captures: CaptureSummary[];
  settings: SettingsView | null;
  error: string | null;

  loadAll: () => Promise<void>;
  selectTask: (id: string | null) => void;
  setCursor: (id: string | null) => void;
  setView: (view: View) => void;
  setGroupBy: (by: GroupBy) => void;
  setQuery: (q: string) => void;
  setShowDone: (v: boolean) => void;
  /**
   * The one path for user status changes that may complete or reopen a task:
   * cascades to the parent, celebrates, then persists. `next` defaults to
   * flipping done/todo.
   */
  toggleDone: (id: string, origin?: Origin, next?: Status) => Promise<void>;
  /** `rank: false` skips the Up next refresh (the caller refreshes it later). */
  patchTask: (id: string, patch: TaskPatch, opts?: { rank?: boolean }) => Promise<Task | null>;
  createQuick: (title: string) => Promise<Task | null>;
  openCapture: () => void;
  closeCapture: () => void;
  refreshActivity: () => Promise<void>;
  loadSettings: () => Promise<void>;
  /** Re-checks the selected provider and refreshes the cached status. */
  probeProvider: () => Promise<void>;
  setError: (message: string | null) => void;
}

export const useStore = create<StoreState>((set, get) => ({
  tasks: [],
  loaded: false,
  showDone: false,
  lingering: [],
  selectedId: null,
  cursorId: null,
  view: "focus",
  groupBy: "category",
  query: "",
  upNext: [],
  captureOpen: false,
  captures: [],
  settings: null,
  error: null,

  async loadAll() {
    try {
      const [tasks, upNext] = await Promise.all([api.listTasks(true), api.upNext(5)]);
      // A lingering row keeps its Up next slot; the linger timer refreshes the ranking.
      set(get().lingering.length ? { tasks, loaded: true } : { tasks, upNext, loaded: true });
    } catch (e) {
      set({ error: api.errorMessage(e), loaded: true });
    }
  },

  selectTask: (id) => set({ selectedId: id, cursorId: id ?? get().cursorId }),
  setCursor: (cursorId) => set({ cursorId }),
  setView: (view) => set({ view, cursorId: null }),
  setGroupBy: (groupBy) => set({ groupBy }),
  setQuery: (query) => set({ query }),
  setShowDone: (showDone) => set({ showDone }),

  async toggleDone(id, origin, next) {
    const tasks = get().tasks;
    const task = tasks.find((t) => t.id === id);
    if (!task) return;
    const to: Status = next ?? (task.status === "done" ? "todo" : "done");
    if (task.status === to) return;

    const decision = decide(tasks, id, to);
    const parentId = decision.completeParentId ?? decision.reopenParentId;
    const parentTo: Status = decision.completeParentId ? "done" : "todo";
    const completed = to === "done" ? [id, ...(decision.completeParentId ? [decision.completeParentId] : [])] : [];
    set({
      tasks: tasks.map((t) =>
        t.id === id ? { ...t, status: to } : t.id === parentId ? { ...t, status: parentTo } : t,
      ),
      lingering: [...get().lingering.filter((x) => !completed.includes(x)), ...completed],
    });
    if (completed.length) {
      setTimeout(() => {
        set({ lingering: get().lingering.filter((x) => !completed.includes(x)) });
        api.upNext(5).then((upNext) => set({ upNext })).catch(() => undefined);
      }, LINGER_MS);
    }

    if (decision.celebrate) {
      combo = nextCombo(combo, Date.now());
      const s = get().settings?.settings;
      celebrate({
        tier: decision.celebrate,
        combo: combo.count,
        origin: origin ?? null,
        sound: s?.sounds ?? true,
        motion: (s?.animations ?? true) && !prefersReducedMotion(),
      });
    }

    // While a completed row lingers, Up next keeps its old ranking; the timer refreshes it.
    const rank = completed.length === 0;
    const updated = await get().patchTask(id, { status: to }, { rank });
    // A failed patch already reloaded the list, which also undoes the parent change.
    if (updated && parentId) await get().patchTask(parentId, { status: parentTo }, { rank });
  },

  async patchTask(id, patch, opts) {
    try {
      const updated = await api.updateTask(id, patch);
      set({ tasks: get().tasks.map((t) => (t.id === id ? updated : t)) });
      if (opts?.rank !== false) api.upNext(5).then((upNext) => set({ upNext })).catch(() => undefined);
      return updated;
    } catch (e) {
      set({ error: api.errorMessage(e) });
      await get().loadAll();
      return null;
    }
  },

  async createQuick(title) {
    try {
      const task = await api.createTask({ title: title.trim() });
      await get().loadAll();
      return task;
    } catch (e) {
      set({ error: api.errorMessage(e) });
      return null;
    }
  },

  openCapture: () => set({ captureOpen: true }),
  closeCapture: () => set({ captureOpen: false }),

  async refreshActivity() {
    try {
      set({ captures: await api.listCaptures(50) });
    } catch (e) {
      set({ error: api.errorMessage(e) });
    }
  },

  async loadSettings() {
    try {
      set({ settings: await api.getSettings() });
    } catch (e) {
      set({ error: api.errorMessage(e) });
    }
  },

  async probeProvider() {
    try {
      await api.probeProvider();
      set({ settings: await api.getSettings() });
    } catch (e) {
      set({ error: api.errorMessage(e) });
    }
  },

  setError: (error) => set({ error }),
}));
