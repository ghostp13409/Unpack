import { describe, expect, it } from "vitest";
import type { Task } from "../lib/types";
import { childrenOf, dueThisWeek, filterTasks, groupTasks, overdue, topLevel, workload } from "./derived";

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

const today = "2026-11-01";

describe("groupTasks", () => {
  it("groups by category with Uncategorized last", () => {
    const tasks = [task("a", { category: "Observability" }), task("b"), task("c", { category: "Admin" })];
    const groups = groupTasks(tasks, "category");
    expect(groups.map((g) => g.label)).toEqual(["Admin", "Observability", "Uncategorized"]);
    expect(groups[2].tasks.map((t) => t.id)).toEqual(["b"]);
  });

  it("groups by priority in urgent-to-low order and skips empty groups", () => {
    const tasks = [task("a", { priority: "low" }), task("b", { priority: "urgent" })];
    const groups = groupTasks(tasks, "priority");
    expect(groups.map((g) => g.label)).toEqual(["Urgent", "Low"]);
  });

  it("groups by due date into buckets", () => {
    const tasks = [
      task("over", { due_date: "2026-10-30" }),
      task("today", { due_date: "2026-11-01" }),
      task("week", { due_date: "2026-11-05" }),
      task("later", { due_date: "2026-12-01" }),
      task("none"),
    ];
    const groups = groupTasks(tasks, "due", today);
    expect(groups.map((g) => g.label)).toEqual(["Overdue", "Today", "This week", "Later", "No date"]);
  });
});

describe("filterTasks", () => {
  it("matches title, category and description, case-insensitively", () => {
    const tasks = [
      task("a", { title: "Send Pricing" }),
      task("b", { category: "Observability" }),
      task("c", { description: "talk to Alex" }),
      task("d"),
    ];
    expect(filterTasks(tasks, "pric").map((t) => t.id)).toEqual(["a"]);
    expect(filterTasks(tasks, "OBSERV").map((t) => t.id)).toEqual(["b"]);
    expect(filterTasks(tasks, "alex").map((t) => t.id)).toEqual(["c"]);
    expect(filterTasks(tasks, "  ").length).toBe(4);
  });
});

describe("overdue / dueThisWeek / workload", () => {
  const tasks = [
    task("o1", { due_date: "2026-10-30", priority: "urgent" }),
    task("t1", { due_date: "2026-11-01" }),
    task("w1", { due_date: "2026-11-07", priority: "high" }),
    task("l1", { due_date: "2026-11-08" }),
    task("d1", { status: "done", completed_at: "2026-10-31T10:00:00Z" }),
    task("d2", { status: "done", completed_at: "2026-10-01T10:00:00Z" }),
    task("p1", { status: "in_progress", priority: "urgent" }),
  ];
  it("overdue excludes done and today", () => {
    expect(overdue(tasks, today).map((t) => t.id)).toEqual(["o1"]);
  });
  it("dueThisWeek covers today through six days out", () => {
    expect(dueThisWeek(tasks, today).map((t) => t.id)).toEqual(["t1", "w1"]);
  });
  it("workload counts open tasks by priority, overdue, and done this week", () => {
    expect(workload(tasks, today)).toEqual({
      open: 5,
      byPriority: { urgent: 2, high: 1, medium: 2, low: 0 },
      overdue: 1,
      doneThisWeek: 1,
    });
  });
});

describe("hierarchy helpers", () => {
  it("childrenOf and topLevel", () => {
    const tasks = [task("p"), task("c", { parent_id: "p" })];
    expect(childrenOf(tasks, "p").map((t) => t.id)).toEqual(["c"]);
    expect(topLevel(tasks).map((t) => t.id)).toEqual(["p"]);
  });
});
