import { addDays } from "../lib/dates";
import type { Task } from "../lib/types";

export interface Stats {
  today: number;
  streak: number;
  best: number;
  week: {
    /** Completion counts for Mon..Fri. */
    days: number[];
    /** Completions Mon..Sun. */
    total: number;
    /** Big tasks completed this week. */
    big: number;
  };
  /** 0..4 for Mon..Fri, -1 on a weekend. */
  todayIndex: number;
}

const pad = (n: number) => String(n).padStart(2, "0");

/** Local YYYY-MM-DD of a timestamp. */
export function localDay(iso: string): string {
  const d = new Date(iso);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** Day of week of a YYYY-MM-DD string, 0 = Sunday .. 6 = Saturday. */
function weekday(day: string): number {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(Date.UTC(y, m - 1, d)).getUTCDay();
}

const isWorkday = (day: string) => {
  const w = weekday(day);
  return w >= 1 && w <= 5;
};

/** The workday strictly before `day`. */
function prevWorkday(day: string): string {
  let d = addDays(day, -1);
  while (!isWorkday(d)) d = addDays(d, -1);
  return d;
}

/** The workday strictly after `day`. */
function nextWorkday(day: string): string {
  let d = addDays(day, 1);
  while (!isWorkday(d)) d = addDays(d, 1);
  return d;
}

/** A completed task whose subtasks (at least one) are all done. */
export function isBigTask(t: Task, all: Task[]): boolean {
  if (t.status !== "done") return false;
  const kids = all.filter((x) => x.parent_id === t.id);
  return kids.length > 0 && kids.every((k) => k.status === "done");
}

export function computeStats(tasks: Task[], now: Date = new Date()): Stats {
  const done = tasks.filter((t) => t.status === "done" && t.completed_at);
  const counts = new Map<string, number>();
  for (const t of done) {
    const day = localDay(t.completed_at as string);
    counts.set(day, (counts.get(day) ?? 0) + 1);
  }

  const today = localDay(now.toISOString());

  // Current streak: today counts if it already has a completion, otherwise
  // start from the previous workday. Weekends are skipped, never break it.
  let streak = 0;
  let cur = isWorkday(today) && counts.has(today) ? today : prevWorkday(today);
  while (counts.has(cur)) {
    streak++;
    cur = prevWorkday(cur);
  }

  // Best streak: longest run of consecutive workdays with a completion.
  const workdays = [...counts.keys()].filter(isWorkday).sort();
  let best = 0;
  let run = 0;
  workdays.forEach((day, i) => {
    run = i > 0 && nextWorkday(workdays[i - 1]) === day ? run + 1 : 1;
    best = Math.max(best, run);
  });
  best = Math.max(best, streak);

  // Week runs Monday..Sunday.
  const todayWeekday = weekday(today);
  const mondayOffset = (todayWeekday + 6) % 7;
  const monday = addDays(today, -mondayOffset);
  const weekDays = Array.from({ length: 7 }, (_, i) => addDays(monday, i));
  const perDay = weekDays.map((d) => counts.get(d) ?? 0);
  const weekSet = new Set(weekDays);
  const big = done.filter((t) => weekSet.has(localDay(t.completed_at as string)) && isBigTask(t, tasks)).length;

  return {
    today: counts.get(today) ?? 0,
    streak,
    best,
    week: { days: perDay.slice(0, 5), total: perDay.reduce((a, b) => a + b, 0), big },
    todayIndex: mondayOffset < 5 ? mondayOffset : -1,
  };
}
