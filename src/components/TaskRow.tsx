import { useEffect, useState } from "react";
import { formatDue, todayISO } from "../lib/dates";
import type { Task } from "../lib/types";
import { useStore } from "../store/useStore";
import { Checkbox } from "./Checkbox";
import { Chip } from "./Chip";
import { PriorityDot } from "./PriorityDot";

interface Props {
  task: Task;
  child?: boolean;
  reason?: string;
  subtasks?: { done: number; total: number };
  /** When true, a task that becomes done fades out before it leaves the list. */
  fadeOnDone?: boolean;
  /** Hide the category chip (the list is already grouped by category). */
  hideCategory?: boolean;
}

export function TaskRow({ task, child, reason, subtasks, fadeOnDone, hideCategory }: Props) {
  const selected = useStore((s) => s.selectedId === task.id);
  const cursor = useStore((s) => s.cursorId === task.id);
  const selectTask = useStore((s) => s.selectTask);
  const toggleDone = useStore((s) => s.toggleDone);
  const [leaving, setLeaving] = useState(false);
  const done = task.status === "done";

  useEffect(() => {
    if (!fadeOnDone || !done) {
      setLeaving(false);
      return;
    }
    const t = setTimeout(() => setLeaving(true), 600);
    return () => clearTimeout(t);
  }, [done, fadeOnDone]);

  const due = task.due_date ? formatDue(task.due_date, todayISO()) : null;

  return (
    <div
      className={`row ${selected ? "selected" : ""} ${cursor ? "cursor" : ""} ${child ? "child" : ""} ${done ? "done" : ""} ${leaving ? "leaving" : ""}`}
      data-task-id={task.id}
      onClick={() => selectTask(task.id)}
    >
      <Checkbox checked={done} onChange={(o) => toggleDone(task.id, o)} />
      <PriorityDot priority={task.priority} />
      <div className="row-main">
        <div className="row-title">{task.title}</div>
        {reason && <div className="row-reason">{reason}</div>}
      </div>
      <div className="row-meta">
        {subtasks && subtasks.total > 0 && (
          <span className="subcount">
            {subtasks.done}/{subtasks.total}
          </span>
        )}
        {task.category && !hideCategory && <Chip>{task.category}</Chip>}
        {due && <span className={`due ${due.tone}`}>{due.label}</span>}
      </div>
    </div>
  );
}
