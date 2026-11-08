import { invoke as tauriInvoke } from "@tauri-apps/api/core";
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

/** True inside the Tauri shell; false in a plain browser (dev preview only). */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type MockModule = typeof import("./mockApi");
let mockPromise: Promise<MockModule["mockApi"]> | null = null;
function mock() {
  mockPromise ??= import("./mockApi").then((m) => m.mockApi);
  return mockPromise;
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) return tauriInvoke<T>(cmd, args);
  const m = await mock();
  const fn = MOCK_MAP[cmd];
  if (!fn) throw `No mock for ${cmd}`;
  return fn(m, args ?? {}) as Promise<T>;
}

type MockFn = (m: MockModule["mockApi"], a: Record<string, unknown>) => Promise<unknown>;
const MOCK_MAP: Record<string, MockFn> = {
  list_tasks: (m, a) => m.listTasks(a.includeDone as boolean),
  get_task_detail: (m, a) => m.getTaskDetail(a.id as string),
  create_task: (m, a) => m.createTask(a.new as NewTask),
  update_task: (m, a) => m.updateTask(a.id as string, a.patch as TaskPatch),
  add_note: (m, a) => m.addNote(a.taskId as string, a.body as string),
  delete_note: (m, a) => m.deleteNote(a.id as string),
  list_categories: (m) => m.listCategories(),
  up_next: (m, a) => m.upNext(a.limit as number),
  start_capture: (m, a) => m.startCapture(a.text as string | null, a.paths as string[]),
  apply_capture: (m, a) => m.applyCapture(a.captureId as string, a.selected as number[]),
  discard_capture: (m, a) => m.discardCapture(a.captureId as string),
  list_captures: (m, a) => m.listCaptures(a.limit as number),
  undo_change: (m, a) => m.undoChange(a.changeId as string, a.force as boolean),
  get_settings: (m) => m.getSettings(),
  save_settings: (m, a) => m.saveSettings(a.settings as Settings),
  set_api_key: (m, a) => m.setApiKey(a.key as string),
  clear_api_key: (m) => m.clearApiKey(),
  test_connection: (m) => m.testConnection(),
  probe_provider: (m) => m.probeProvider(),
  open_data_folder: (m) => m.openDataFolder(),
};

export const listTasks = (includeDone: boolean) => invoke<Task[]>("list_tasks", { includeDone });
export const getTaskDetail = (id: string) => invoke<TaskDetail>("get_task_detail", { id });
export const createTask = (task: NewTask) => invoke<Task>("create_task", { new: task });
export const updateTask = (id: string, patch: TaskPatch) => invoke<Task>("update_task", { id, patch });
export const addNote = (taskId: string, body: string) => invoke<Note>("add_note", { taskId, body });
export const deleteNote = (id: string) => invoke<void>("delete_note", { id });
export const listCategories = () => invoke<string[]>("list_categories");
export const upNext = (limit: number) => invoke<Ranked[]>("up_next", { limit });

export const startCapture = (text: string | null, paths: string[]) =>
  invoke<Capture>("start_capture", { text, paths });
export const applyCapture = (captureId: string, selected: number[]) =>
  invoke<Change[]>("apply_capture", { captureId, selected });
export const discardCapture = (captureId: string) => invoke<void>("discard_capture", { captureId });
export const listCaptures = (limit: number) => invoke<CaptureSummary[]>("list_captures", { limit });
export const undoChange = (changeId: string, force: boolean) =>
  invoke<UndoOutcome>("undo_change", { changeId, force });

export const getSettings = () => invoke<SettingsView>("get_settings");
export const saveSettings = (settings: Settings) => invoke<void>("save_settings", { settings });
export const setApiKey = (key: string) => invoke<void>("set_api_key", { key });
export const clearApiKey = () => invoke<void>("clear_api_key");
export const testConnection = () => invoke<string>("test_connection");
export const probeProvider = () => invoke<ProviderStatus>("probe_provider");
export const openDataFolder = () => invoke<void>("open_data_folder");

// Autostart lives in the Tauri plugin (a Run registry entry), not in our own commands.
export async function autostartEnabled(): Promise<boolean> {
  if (inTauri) {
    const { isEnabled } = await import("@tauri-apps/plugin-autostart");
    return isEnabled();
  }
  return (await mock()).autostartEnabled();
}
export async function setAutostart(on: boolean): Promise<void> {
  if (inTauri) {
    const { enable, disable } = await import("@tauri-apps/plugin-autostart");
    return on ? enable() : disable();
  }
  return (await mock()).setAutostart(on);
}

/** Tauri rejects commands with a plain string; normalize anything else. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Something went wrong.";
}
