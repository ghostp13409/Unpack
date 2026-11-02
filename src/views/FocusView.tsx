import { ChevronRight } from "lucide-react";
import { useMemo, useState } from "react";
import { EmptyState } from "../components/EmptyState";
import { TaskRow } from "../components/TaskRow";
import { WorkloadStrip } from "../components/WorkloadStrip";
import { todayISO } from "../lib/dates";
import type { Task } from "../lib/types";
import { dueThisWeek, isOpen, overdue, workload } from "../store/derived";
import { useStore } from "../store/useStore";

function Collapsible({ title, tasks, allTasks, defaultOpen }: { title: string; tasks: Task[]; allTasks: Task[]; defaultOpen: boolean }) {
  const [open, setOpen] = useState(defaultOpen);
  if (tasks.length === 0) return null;
  return (
    <section className={`group collapsible ${open ? "open" : ""}`}>
      <div className="group-title" onClick={() => setOpen((v) => !v)}>
        <ChevronRight size={14} />
        <h3>{title}</h3>
        <span className="count">{tasks.length}</span>
      </div>
      {open && tasks.map((t) => <TaskRow key={t.id} task={t} subtasks={subCount(allTasks, t.id)} fadeOnDone />)}
    </section>
  );
}

function subCount(all: Task[], id: string) {
  const kids = all.filter((t) => t.parent_id === id && t.status !== "dropped");
  return { done: kids.filter((k) => k.status === "done").length, total: kids.length };
}

export function FocusView() {
  const tasks = useStore((s) => s.tasks);
  const loaded = useStore((s) => s.loaded);
  const upNext = useStore((s) => s.upNext);
  const today = todayISO();

  const byId = useMemo(() => new Map(tasks.map((t) => [t.id, t])), [tasks]);
  const next = upNext.map((r) => ({ task: byId.get(r.task_id), reasons: r.reasons })).filter((x) => x.task && isOpen(x.task));
  const over = overdue(tasks, today);
  const week = dueThisWeek(tasks, today);
  const load = workload(tasks, today);
  const greeting = (() => {
    const h = new Date().getHours();
    return h < 12 ? "Good morning" : h < 18 ? "Good afternoon" : "Good evening";
  })();

  return (
    <div className="page">
      <div className="page-inner">
        <header className="page-header">
          <div>
            <h1>Focus</h1>
            <div className="page-sub">
              {greeting}. {load.open === 0 ? "Nothing open." : load.open === 1 ? "One thing open." : `${load.open} things open.`}
            </div>
          </div>
        </header>

        {loaded && load.open === 0 ? (
          <EmptyState message="All clear. Capture what's next." />
        ) : (
          <>
            <section className="group">
              <div className="group-title">
                <h3>Up next</h3>
              </div>
              {next.map(({ task, reasons }) => (
                <TaskRow key={task!.id} task={task!} reason={reasons.join(" · ")} subtasks={subCount(tasks, task!.id)} fadeOnDone />
              ))}
              {next.length === 0 && <div className="muted small" style={{ padding: "8px" }}>Nothing ranked yet.</div>}
            </section>

            <Collapsible title="Overdue" tasks={over} allTasks={tasks} defaultOpen={true} />
            <Collapsible title="Due this week" tasks={week} allTasks={tasks} defaultOpen={false} />

            <WorkloadStrip workload={load} />
          </>
        )}
      </div>
    </div>
  );
}
