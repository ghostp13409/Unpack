import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SettingsView, Task, TaskPatch } from "../lib/types";

vi.mock("../lib/api", () => ({
  updateTask: vi.fn(),
  upNext: vi.fn(async () => []),
  listTasks: vi.fn(async () => []),
  errorMessage: (e: unknown) => String(e),
}));
vi.mock("../lib/celebrate", () => ({ celebrate: vi.fn() }));

import * as api from "../lib/api";
import { celebrate } from "../lib/celebrate";
import { useStore } from "./useStore";

function task(id: string, over: Partial<Task> = {}): Task {
  return {
    id,
    title: id,
    description: "",
    status: "todo",
    priority: "medium",
    category: null,
    due_date: null,
    parent_id: null,
    pinned: false,
    snoozed_until: null,
    created_at: "2026-11-01T00:00:00Z",
    updated_at: "2026-11-01T00:00:00Z",
    completed_at: null,
    ...over,
  };
}

function settings(sounds: boolean, animations: boolean): SettingsView {
  return {
    settings: {
      model: "",
      effort: "",
      user_name: "",
      manager_name: "",
      theme: "system",
      provider: "",
      claude_code_path: "",
      sounds,
      animations,
    },
    has_key: false,
    key_hint: null,
    data_dir: "",
    provider_status: {} as SettingsView["provider_status"],
  };
}

let reducedMotion = false;

function seed(tasks: Task[], s = settings(true, true)) {
  useStore.setState({ tasks, settings: s, error: null });
}

const status = (id: string) => useStore.getState().tasks.find((t) => t.id === id)?.status;
const updatedIds = () => vi.mocked(api.updateTask).mock.calls.map((c) => c[0]);

beforeEach(() => {
  reducedMotion = false;
  vi.stubGlobal("window", {
    matchMedia: (q: string) => ({ matches: q === "(prefers-reduced-motion: reduce)" && reducedMotion }),
  });
  vi.mocked(celebrate).mockClear();
  vi.mocked(api.updateTask).mockReset();
  vi.mocked(api.updateTask).mockImplementation(async (id: string, patch: TaskPatch) => {
    const t = useStore.getState().tasks.find((x) => x.id === id)!;
    return { ...t, ...patch } as Task;
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("toggleDone", () => {
  it("completing last subtask marks parent done and celebrates big once", async () => {
    seed([task("P"), task("A", { parent_id: "P", status: "done" }), task("B", { parent_id: "P" })]);
    await useStore.getState().toggleDone("B");
    expect(status("B")).toBe("done");
    expect(status("P")).toBe("done");
    expect(updatedIds()).toEqual(expect.arrayContaining(["B", "P"]));
    expect(api.updateTask).toHaveBeenCalledWith("P", { status: "done" });
    expect(celebrate).toHaveBeenCalledTimes(1);
    expect(vi.mocked(celebrate).mock.calls[0][0]).toMatchObject({ tier: "big" });
  });

  it("un-checking subtask reopens parent and does not celebrate", async () => {
    seed([task("P", { status: "done" }), task("A", { parent_id: "P", status: "done" })]);
    await useStore.getState().toggleDone("A");
    expect(status("A")).toBe("todo");
    expect(status("P")).toBe("todo");
    expect(api.updateTask).toHaveBeenCalledWith("P", { status: "todo" });
    expect(celebrate).not.toHaveBeenCalled();
  });

  it("double toggle to done celebrates once", async () => {
    seed([task("T")]);
    await useStore.getState().toggleDone("T", undefined, "done");
    await useStore.getState().toggleDone("T", undefined, "done");
    expect(status("T")).toBe("done");
    expect(celebrate).toHaveBeenCalledTimes(1);
  });

  it("status select done->in_progress reopens parent", async () => {
    seed([task("P", { status: "done" }), task("A", { parent_id: "P", status: "done" })]);
    await useStore.getState().toggleDone("A", undefined, "in_progress");
    expect(status("A")).toBe("in_progress");
    expect(status("P")).toBe("todo");
    expect(celebrate).not.toHaveBeenCalled();
  });

  it("respects settings flags", async () => {
    seed([task("T")], settings(false, false));
    await useStore.getState().toggleDone("T", { x: 10, y: 20 });
    expect(vi.mocked(celebrate).mock.calls[0][0]).toMatchObject({
      tier: "task",
      sound: false,
      motion: false,
      origin: { x: 10, y: 20 },
    });

    vi.mocked(celebrate).mockClear();
    reducedMotion = true;
    seed([task("U")], settings(true, true));
    await useStore.getState().toggleDone("U");
    expect(vi.mocked(celebrate).mock.calls[0][0]).toMatchObject({ sound: true, motion: false, origin: null });
  });

  it("combo increments within 10s", async () => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2030-01-01T09:00:00Z"));
    seed([task("X"), task("Y")]);
    await useStore.getState().toggleDone("X");
    vi.setSystemTime(new Date("2030-01-01T09:00:03Z"));
    await useStore.getState().toggleDone("Y");
    const calls = vi.mocked(celebrate).mock.calls;
    expect(calls).toHaveLength(2);
    expect(calls[0][0]).toMatchObject({ combo: 0 });
    expect(calls[1][0]).toMatchObject({ combo: 1 });
  });
});
