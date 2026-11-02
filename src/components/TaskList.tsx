import type { Task } from "../lib/types";
import type { TaskGroup } from "../store/derived";
import { TaskRow } from "./TaskRow";

/** Orders a group's tasks so children follow their parent (one level). */
function orderWithChildren(tasks: Task[]): { task: Task; child: boolean }[] {
  const ids = new Set(tasks.map((t) => t.id));
  const out: { task: Task; child: boolean }[] = [];
  const seen = new Set<string>();
  for (const t of tasks) {
    if (t.parent_id && ids.has(t.parent_id)) continue;
    out.push({ task: t, child: false });
    seen.add(t.id);
    for (const c of tasks) {
      if (c.parent_id === t.id && !seen.has(c.id)) {
        out.push({ task: c, child: true });
        seen.add(c.id);
      }
    }
  }
  return out;
}

export function TaskList({ groups, allTasks, fadeOnDone, hideCategory }: { groups: TaskGroup[]; allTasks: Task[]; fadeOnDone?: boolean; hideCategory?: boolean }) {
  const subtaskCount = (id: string) => {
    const kids = allTasks.filter((t) => t.parent_id === id && t.status !== "dropped");
    return { done: kids.filter((k) => k.status === "done").length, total: kids.length };
  };
  return (
    <>
      {groups.map((g) => (
        <section className="group" key={g.key}>
          <div className="group-title">
            <h3>{g.label}</h3>
            <span className="count">{g.tasks.length}</span>
          </div>
          {orderWithChildren(g.tasks).map(({ task, child }) => (
            <TaskRow key={task.id} task={task} child={child} subtasks={subtaskCount(task.id)} fadeOnDone={fadeOnDone} hideCategory={hideCategory} />
          ))}
        </section>
      ))}
    </>
  );
}
