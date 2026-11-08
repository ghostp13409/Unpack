import { describe, expect, it } from "vitest";
import type { Task } from "../lib/types";
import { computeStats, isBigTask, localDay } from "./stats";

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

const at = (y: number, mon: number, d: number, h = 12, min = 0) => new Date(y, mon - 1, d, h, min).toISOString();
const now = new Date(2026, 10, 11, 15); // Wednesday 11 Nov 2026

let n = 0;
function doneOn(y: number, mon: number, d: number, over: Partial<Task> = {}): Task {
  return task(`t${n++}`, { status: "done", completed_at: at(y, mon, d), ...over });
}
const doneNov = (...days: number[]) => days.map((d) => doneOn(2026, 11, d));

describe("localDay", () => {
  it("returns the local YYYY-MM-DD of a timestamp", () => {
    expect(localDay(at(2026, 11, 9, 23, 30))).toBe("2026-11-09");
    expect(localDay(at(2026, 1, 2, 0, 5))).toBe("2026-01-02");
  });
});

describe("isBigTask", () => {
  it("needs a done status, at least one child and all children done", () => {
    const p = task("p", { status: "done" });
    const a = task("a", { status: "done", parent_id: "p" });
    const b = task("b", { status: "todo", parent_id: "p" });
    expect(isBigTask(p, [p])).toBe(false);
    expect(isBigTask(p, [p, a])).toBe(true);
    expect(isBigTask(p, [p, a, b])).toBe(false);
    expect(isBigTask(task("q"), [a])).toBe(false);
  });
});

describe("computeStats", () => {
  it("today counts tasks done today including subtasks", () => {
    const tasks = [
      doneOn(2026, 11, 11),
      doneOn(2026, 11, 11, { parent_id: "x" }),
      doneOn(2026, 11, 10),
    ];
    expect(computeStats(tasks, now).today).toBe(2);
  });

  it("uses local day not UTC", () => {
    const t = task("late", { status: "done", completed_at: at(2026, 11, 9, 23, 30) });
    expect(computeStats([t], now).week.days[0]).toBe(1);
  });

  it("streak counts consecutive workdays back from today", () => {
    expect(computeStats(doneNov(9, 10, 11), now).streak).toBe(3);
  });

  it("weekend never breaks streak", () => {
    expect(computeStats(doneNov(5, 6, 9, 10, 11), now).streak).toBe(5);
  });

  it("today without completion does not break streak", () => {
    expect(computeStats(doneNov(9, 10), now).streak).toBe(2);
  });

  it("gap on a workday breaks streak", () => {
    expect(computeStats(doneNov(9, 11), now).streak).toBe(1);
  });

  it("weekend completion does not extend streak", () => {
    expect(computeStats(doneNov(7, 8), now).streak).toBe(0);
  });

  it("best streak is longest run in history", () => {
    const october = [12, 13, 14, 15].map((d) => doneOn(2026, 10, d));
    const s = computeStats([...october, ...doneNov(9, 10)], now);
    expect(s.streak).toBe(2);
    expect(s.best).toBe(4);
  });

  it("best equals streak when the current run is longer", () => {
    const october = [12, 13].map((d) => doneOn(2026, 10, d));
    const s = computeStats([...october, ...doneNov(9, 10, 11)], now);
    expect(s.best).toBe(3);
    expect(s.streak).toBe(3);
  });

  it("best streak bridges weekends", () => {
    const s = computeStats(doneNov(5, 6, 9), new Date(2026, 10, 20, 12));
    expect(s.best).toBe(3);
    expect(s.streak).toBe(0);
  });

  it("week totals include weekend, chart is Mon-Fri", () => {
    const s = computeStats([doneOn(2026, 11, 9), doneOn(2026, 11, 15)], now);
    expect(s.week.total).toBe(2);
    expect(s.week.days).toEqual([1, 0, 0, 0, 0]);
  });

  it("excludes completions outside the current week from week totals", () => {
    const s = computeStats([doneOn(2026, 11, 8), doneOn(2026, 11, 16)], now);
    expect(s.week.total).toBe(0);
  });

  it("big task counted when all children done", () => {
    const parent = doneOn(2026, 11, 10, { id: "p" });
    const kids = [doneOn(2026, 11, 9, { parent_id: "p" }), doneOn(2026, 11, 10, { parent_id: "p" })];
    const childless = doneOn(2026, 11, 10, { id: "solo" });
    expect(computeStats([parent, ...kids], now).week.big).toBe(1);
    expect(computeStats([childless], now).week.big).toBe(0);
  });

  it("ignores not-done tasks with stale completed_at", () => {
    const t = task("stale", { status: "todo", completed_at: at(2026, 11, 11) });
    const s = computeStats([t], now);
    expect(s.today).toBe(0);
    expect(s.week.total).toBe(0);
    expect(s.streak).toBe(0);
  });

  it("todayIndex is Mon-Fri index, -1 on weekend, and streak follows Friday", () => {
    expect(computeStats([], now).todayIndex).toBe(2);
    const sat = new Date(2026, 10, 14, 12);
    const s = computeStats(doneNov(5, 6, 9, 10, 11, 12, 13), sat);
    expect(s.todayIndex).toBe(-1);
    expect(s.streak).toBe(7);
    expect(computeStats(doneNov(9, 10, 11), sat).streak).toBe(0);
  });
});
