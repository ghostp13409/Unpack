import type { Status, Task } from "../types";

export type Tier = "subtask" | "task" | "big";

export interface Decision {
  celebrate: Tier | null;
  completeParentId: string | null;
  reopenParentId: string | null;
}

export const COMBO_WINDOW_MS = 10_000;

const NONE: Decision = { celebrate: null, completeParentId: null, reopenParentId: null };

/** Live children of a task: dropped ones are ignored. */
const liveChildren = (tasks: Task[], parentId: string) =>
  tasks.filter((t) => t.parent_id === parentId && t.status !== "dropped");

/**
 * Decide what a status change should celebrate and whether it cascades to the parent.
 * `tasks` is the state before the change.
 */
export function decide(tasks: Task[], id: string, next: Status): Decision {
  const t = tasks.find((x) => x.id === id);
  if (!t || t.status === next) return NONE;

  if (next !== "done") {
    if (t.status !== "done" || !t.parent_id) return NONE;
    const parent = tasks.find((x) => x.id === t.parent_id);
    return parent?.status === "done" ? { ...NONE, reopenParentId: parent.id } : NONE;
  }

  if (t.parent_id) {
    const parent = tasks.find((x) => x.id === t.parent_id);
    const siblingsDone = liveChildren(tasks, t.parent_id)
      .filter((x) => x.id !== id)
      .every((x) => x.status === "done");
    if (parent && parent.status !== "done" && parent.status !== "dropped" && siblingsDone) {
      return { celebrate: "big", completeParentId: parent.id, reopenParentId: null };
    }
    return { ...NONE, celebrate: "subtask" };
  }

  const kids = liveChildren(tasks, t.id);
  const big = kids.length > 0 && kids.every((x) => x.status === "done");
  return { ...NONE, celebrate: big ? "big" : "task" };
}

/** Combo count is 0 for the first completion, +1 for each within the window. */
export function nextCombo(
  prev: { at: number; count: number } | null,
  now: number,
): { at: number; count: number } {
  if (prev && now - prev.at < COMBO_WINDOW_MS) return { at: now, count: prev.count + 1 };
  return { at: now, count: 0 };
}
