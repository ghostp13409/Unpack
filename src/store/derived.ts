import { daysBetween, todayISO } from "../lib/dates";
import type { Priority, Task } from "../lib/types";
import { PRIORITIES, PRIORITY_LABEL } from "../lib/types";

export type GroupBy = "category" | "priority" | "due";

export interface TaskGroup {
  key: string;
  label: string;
  tasks: Task[];
}

export function isOpen(t: Task): boolean {
  return t.status === "todo" || t.status === "in_progress";
}

export function groupTasks(tasks: Task[], by: GroupBy, today: string = todayISO()): TaskGroup[] {
  if (by === "priority") {
    return PRIORITIES.map((p) => ({
      key: p,
      label: PRIORITY_LABEL[p],
      tasks: tasks.filter((t) => t.priority === p),
    })).filter((g) => g.tasks.length > 0);
  }
  if (by === "due") {
    const buckets: TaskGroup[] = [
      { key: "overdue", label: "Overdue", tasks: [] },
      { key: "today", label: "Today", tasks: [] },
      { key: "week", label: "This week", tasks: [] },
      { key: "later", label: "Later", tasks: [] },
      { key: "none", label: "No date", tasks: [] },
    ];
    for (const t of tasks) {
      if (!t.due_date) {
        buckets[4].tasks.push(t);
        continue;
      }
      const d = daysBetween(today, t.due_date);
      if (d < 0) buckets[0].tasks.push(t);
      else if (d === 0) buckets[1].tasks.push(t);
      else if (d <= 6) buckets[2].tasks.push(t);
      else buckets[3].tasks.push(t);
    }
    return buckets.filter((g) => g.tasks.length > 0);
  }
  const map = new Map<string, Task[]>();
  for (const t of tasks) {
    const key = t.category?.trim() || "";
    if (!map.has(key)) map.set(key, []);
    map.get(key)!.push(t);
  }
  const named = [...map.keys()]
    .filter((k) => k !== "")
    .sort((a, b) => a.localeCompare(b, undefined, { sensitivity: "base" }));
  const groups: TaskGroup[] = named.map((k) => ({ key: k, label: k, tasks: map.get(k)! }));
  if (map.has("")) groups.push({ key: "", label: "Uncategorized", tasks: map.get("")! });
  return groups;
}

export function filterTasks(tasks: Task[], query: string): Task[] {
  const q = query.trim().toLowerCase();
  if (!q) return tasks;
  return tasks.filter(
    (t) =>
      t.title.toLowerCase().includes(q) ||
      (t.category ?? "").toLowerCase().includes(q) ||
      t.description.toLowerCase().includes(q),
  );
}

export function overdue(tasks: Task[], today: string = todayISO()): Task[] {
  return tasks.filter((t) => isOpen(t) && t.due_date !== null && daysBetween(today, t.due_date) < 0);
}

export function dueThisWeek(tasks: Task[], today: string = todayISO()): Task[] {
  return tasks.filter((t) => {
    if (!isOpen(t) || !t.due_date) return false;
    const d = daysBetween(today, t.due_date);
    return d >= 0 && d <= 6;
  });
}

export interface Workload {
  open: number;
  byPriority: Record<Priority, number>;
  overdue: number;
  doneThisWeek: number;
}

export function workload(tasks: Task[], today: string = todayISO()): Workload {
  const open = tasks.filter(isOpen);
  const byPriority: Record<Priority, number> = { urgent: 0, high: 0, medium: 0, low: 0 };
  for (const t of open) byPriority[t.priority] += 1;
  const doneThisWeek = tasks.filter((t) => {
    if (t.status !== "done" || !t.completed_at) return false;
    const d = daysBetween(t.completed_at.slice(0, 10), today);
    return d >= 0 && d <= 6;
  }).length;
  return { open: open.length, byPriority, overdue: overdue(tasks, today).length, doneThisWeek };
}

export function childrenOf(tasks: Task[], parentId: string): Task[] {
  return tasks.filter((t) => t.parent_id === parentId);
}

export function topLevel(tasks: Task[]): Task[] {
  return tasks.filter((t) => t.parent_id === null);
}
