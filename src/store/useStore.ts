import { create } from "zustand";
import * as api from "../lib/api";
import type { CaptureSummary, Ranked, SettingsView, Task, TaskPatch } from "../lib/types";
import type { GroupBy } from "./derived";

export type View = "focus" | "tasks" | "activity" | "settings";

interface StoreState {
  tasks: Task[];
  loaded: boolean;
  showDone: boolean;
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
  toggleDone: (id: string) => Promise<void>;
  patchTask: (id: string, patch: TaskPatch) => Promise<Task | null>;
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
      set({ tasks, upNext, loaded: true });
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

  async toggleDone(id) {
    const task = get().tasks.find((t) => t.id === id);
    if (!task) return;
    const next = task.status === "done" ? "todo" : "done";
    set({ tasks: get().tasks.map((t) => (t.id === id ? { ...t, status: next } : t)) });
    await get().patchTask(id, { status: next });
  },

  async patchTask(id, patch) {
    try {
      const updated = await api.updateTask(id, patch);
      set({ tasks: get().tasks.map((t) => (t.id === id ? updated : t)) });
      api.upNext(5).then((upNext) => set({ upNext })).catch(() => undefined);
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
