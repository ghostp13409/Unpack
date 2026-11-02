import { Search } from "lucide-react";
import { useMemo } from "react";
import { EmptyState } from "../components/EmptyState";
import { TaskList } from "../components/TaskList";
import { filterTasks, groupTasks, isOpen, type GroupBy } from "../store/derived";
import { useStore } from "../store/useStore";

const GROUPS: { key: GroupBy; label: string }[] = [
  { key: "category", label: "Category" },
  { key: "priority", label: "Priority" },
  { key: "due", label: "Due" },
];

export function TasksView() {
  const tasks = useStore((s) => s.tasks);
  const loaded = useStore((s) => s.loaded);
  const query = useStore((s) => s.query);
  const setQuery = useStore((s) => s.setQuery);
  const groupBy = useStore((s) => s.groupBy);
  const setGroupBy = useStore((s) => s.setGroupBy);
  const showDone = useStore((s) => s.showDone);
  const setShowDone = useStore((s) => s.setShowDone);

  const groups = useMemo(() => {
    const visible = tasks.filter((t) => (showDone ? t.status !== "dropped" : isOpen(t)));
    return groupTasks(filterTasks(visible, query), groupBy);
  }, [tasks, query, groupBy, showDone]);

  const openCount = tasks.filter(isOpen).length;

  return (
    <div className="page">
      <div className="page-inner">
        <header className="page-header">
          <div>
            <h1>Tasks</h1>
            <div className="page-sub">{openCount === 1 ? "1 open task" : `${openCount} open tasks`}</div>
          </div>
          <div className="search">
            <Search size={14} />
            <input
              id="task-search"
              className="input"
              placeholder="Search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <div className="segmented" role="tablist" aria-label="Group by">
            {GROUPS.map((g) => (
              <button key={g.key} className={groupBy === g.key ? "active" : ""} onClick={() => setGroupBy(g.key)}>
                {g.label}
              </button>
            ))}
          </div>
          <label className="toggle">
            <input type="checkbox" checked={showDone} onChange={(e) => setShowDone(e.target.checked)} /> Show done
          </label>
        </header>

        {loaded && groups.length === 0 ? (
          <EmptyState message={query ? "No tasks match that search." : "Nothing here yet. Capture something."} />
        ) : (
          <TaskList groups={groups} allTasks={tasks} fadeOnDone={!showDone} hideCategory={groupBy === "category"} />
        )}
      </div>
    </div>
  );
}
