import { describe, expect, it } from "vitest";
import type { Status, Task } from "../types";
import { COMBO_WINDOW_MS, decide, nextCombo } from "./decide";

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

const NONE = { celebrate: null, completeParentId: null, reopenParentId: null };

describe("decide", () => {
  it("top-level task todo->done is task tier", () => {
    expect(decide([task("a")], "a", "done")).toEqual({ celebrate: "task", completeParentId: null, reopenParentId: null });
  });

  it("subtask done while siblings open is subtask tier", () => {
    const tasks = [task("P"), task("A", { parent_id: "P" }), task("B", { parent_id: "P" })];
    expect(decide(tasks, "A", "done")).toEqual({ celebrate: "subtask", completeParentId: null, reopenParentId: null });
  });

  it("last open subtask completes parent with big tier", () => {
    const tasks = [task("P"), task("A", { parent_id: "P", status: "done" }), task("B", { parent_id: "P" })];
    expect(decide(tasks, "B", "done")).toEqual({ celebrate: "big", completeParentId: "P", reopenParentId: null });
  });

  it("last subtask under an already-done parent is subtask tier and completes nothing", () => {
    const tasks = [task("P", { status: "done" }), task("A", { parent_id: "P", status: "done" }), task("B", { parent_id: "P" })];
    expect(decide(tasks, "B", "done")).toEqual({ celebrate: "subtask", completeParentId: null, reopenParentId: null });
  });

  it("completing parent whose children are all done is big", () => {
    const tasks = [task("P"), task("A", { parent_id: "P", status: "done" }), task("B", { parent_id: "P", status: "done" })];
    expect(decide(tasks, "P", "done")).toEqual({ celebrate: "big", completeParentId: null, reopenParentId: null });
  });

  it("completing parent with open children is task tier", () => {
    const tasks = [task("P"), task("A", { parent_id: "P", status: "done" }), task("B", { parent_id: "P" })];
    expect(decide(tasks, "P", "done")).toEqual({ celebrate: "task", completeParentId: null, reopenParentId: null });
  });

  it("un-checking anything never celebrates", () => {
    expect(decide([task("a", { status: "done" })], "a", "todo")).toEqual(NONE);
  });

  it("un-checking a subtask reopens a done parent", () => {
    const tasks = [task("P", { status: "done" }), task("A", { parent_id: "P", status: "done" })];
    expect(decide(tasks, "A", "todo")).toEqual({ celebrate: null, completeParentId: null, reopenParentId: "P" });
  });

  it("un-checking a subtask leaves an open parent alone", () => {
    const tasks = [task("P"), task("A", { parent_id: "P", status: "done" })];
    expect(decide(tasks, "A", "todo")).toEqual(NONE);
  });

  it("done to in_progress counts as un-check", () => {
    const tasks = [task("P", { status: "done" }), task("A", { parent_id: "P", status: "done" })];
    const next: Status = "in_progress";
    expect(decide(tasks, "A", next)).toEqual({ celebrate: null, completeParentId: null, reopenParentId: "P" });
  });

  it("no-op when already in target state", () => {
    const tasks = [task("P"), task("A", { parent_id: "P", status: "done" })];
    expect(decide(tasks, "A", "done")).toEqual(NONE);
  });

  it("no-op for an unknown id", () => {
    expect(decide([task("a")], "zzz", "done")).toEqual(NONE);
  });

  it("dropped siblings are ignored when checking all-done", () => {
    const tasks = [
      task("P"),
      task("A", { parent_id: "P", status: "done" }),
      task("X", { parent_id: "P", status: "dropped" }),
      task("B", { parent_id: "P" }),
    ];
    expect(decide(tasks, "B", "done")).toEqual({ celebrate: "big", completeParentId: "P", reopenParentId: null });
  });
});

describe("nextCombo", () => {
  it("starts at count 0", () => {
    expect(nextCombo(null, 1000)).toEqual({ at: 1000, count: 0 });
  });

  it("increments within the window", () => {
    expect(nextCombo({ at: 1000, count: 2 }, 1000 + COMBO_WINDOW_MS - 1)).toEqual({ at: 1000 + COMBO_WINDOW_MS - 1, count: 3 });
  });

  it("resets at exactly the window", () => {
    expect(nextCombo({ at: 1000, count: 2 }, 1000 + COMBO_WINDOW_MS)).toEqual({ at: 1000 + COMBO_WINDOW_MS, count: 0 });
  });
});
