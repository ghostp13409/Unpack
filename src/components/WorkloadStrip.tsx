import type { Workload } from "../store/derived";

export function WorkloadStrip({ workload }: { workload: Workload }) {
  const parts: string[] = [];
  parts.push(workload.open === 1 ? "1 open" : `${workload.open} open`);
  if (workload.byPriority.urgent) parts.push(`${workload.byPriority.urgent} urgent`);
  if (workload.byPriority.high) parts.push(`${workload.byPriority.high} high`);
  if (workload.overdue) parts.push(`${workload.overdue} overdue`);
  if (workload.doneThisWeek) parts.push(`${workload.doneThisWeek} done this week`);
  return (
    <div className="strip" aria-label="Workload">
      {parts.map((p, i) => (
        <span key={p}>
          {i > 0 && <span className="sep">&nbsp;&middot;&nbsp;</span>}
          {p}
        </span>
      ))}
    </div>
  );
}
