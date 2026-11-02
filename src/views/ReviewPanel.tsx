import { useMemo, useState } from "react";
import { Button } from "../components/Button";
import { Chip } from "../components/Chip";
import { PriorityDot } from "../components/PriorityDot";
import * as api from "../lib/api";
import { shortDate } from "../lib/dates";
import type { Capture, ProposedOp, TaskPatch } from "../lib/types";
import { PRIORITY_LABEL } from "../lib/types";
import { useStore } from "../store/useStore";

const FIELD_LABEL: Record<string, string> = {
  title: "Title",
  description: "Description",
  priority: "Priority",
  category: "Category",
  due_date: "Due",
  parent_id: "Parent",
};

function fmt(field: string, v: unknown, titleOf: (id: string) => string): string {
  if (v === null || v === undefined || v === "") return "none";
  if (field === "priority") return PRIORITY_LABEL[v as keyof typeof PRIORITY_LABEL] ?? String(v);
  if (field === "due_date") return shortDate(String(v));
  if (field === "parent_id") return titleOf(String(v));
  const s = String(v);
  return s.length > 60 ? s.slice(0, 60) + "…" : s;
}

export function ReviewPanel({ capture, onDone }: { capture: Capture; onDone: () => void }) {
  const tasks = useStore((s) => s.tasks);
  const loadAll = useStore((s) => s.loadAll);
  const selectTask = useStore((s) => s.selectTask);
  const setError = useStore((s) => s.setError);
  const proposal = capture.proposal!;
  const [selected, setSelected] = useState<Set<number>>(() => new Set(proposal.ops.map((o) => o.index)));
  const [busy, setBusy] = useState(false);

  const titleOf = (id: string) => tasks.find((t) => t.id === id)?.title ?? "a task";
  const groups = useMemo(() => {
    const creates = proposal.ops.filter((o) => o.op.type === "create");
    const updates = proposal.ops.filter((o) => o.op.type === "update");
    const completes = proposal.ops.filter((o) => o.op.type === "complete");
    const notes = proposal.ops.filter((o) => o.op.type === "add_note");
    // Parents first, each followed by its temp children.
    const ordered: { op: ProposedOp; child: boolean }[] = [];
    const seen = new Set<number>();
    for (const c of creates) {
      if (c.op.type !== "create" || c.op.parent_temp) continue;
      ordered.push({ op: c, child: false });
      seen.add(c.index);
      for (const k of creates) {
        if (k.op.type === "create" && k.op.parent_temp && k.op.parent_temp === c.op.temp_id) {
          ordered.push({ op: k, child: true });
          seen.add(k.index);
        }
      }
    }
    for (const c of creates) if (!seen.has(c.index)) ordered.push({ op: c, child: false });
    return { creates: ordered, updates, completes, notes };
  }, [proposal]);

  const toggle = (i: number) =>
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(i)) n.delete(i);
      else n.add(i);
      return n;
    });

  const apply = async () => {
    setBusy(true);
    try {
      const changes = await api.applyCapture(capture.id, [...selected]);
      await loadAll();
      const first = changes.find((c) => c.kind === "create")?.task_id ?? changes[0]?.task_id ?? null;
      if (first) selectTask(first);
      onDone();
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const discard = async () => {
    try {
      await api.discardCapture(capture.id);
    } catch {
      /* already gone */
    }
    onDone();
  };

  const count = selected.size;
  const nothing = proposal.ops.length === 0;

  return (
    <>
      <p className="review-summary">{proposal.summary}</p>
      {proposal.warnings.map((w, i) => (
        <div className="review-warn" key={i}>
          {w}
        </div>
      ))}

      {groups.creates.length > 0 && (
        <div className="review-group">
          <h3>New tasks</h3>
          {groups.creates.map(({ op, child }) => {
            if (op.op.type !== "create") return null;
            const n = op.op.new;
            return (
              <label className={`review-item ${child ? "child" : ""}`} key={op.index}>
                <input type="checkbox" checked={selected.has(op.index)} onChange={() => toggle(op.index)} />
                <div className="ri-body">
                  <div className="ri-title">
                    <PriorityDot priority={n.priority} />
                    <span>{n.title}</span>
                    {n.category && <Chip>{n.category}</Chip>}
                    {n.due_date && <span className="xs muted">{shortDate(n.due_date)}</span>}
                    {n.parent_id && <span className="xs muted">under {titleOf(n.parent_id)}</span>}
                  </div>
                  {n.description && <div className="ri-diff">{n.description}</div>}
                  <div className="ri-reason">{op.reason}</div>
                </div>
              </label>
            );
          })}
        </div>
      )}

      {groups.updates.length > 0 && (
        <div className="review-group">
          <h3>Updates</h3>
          {groups.updates.map((op) => {
            if (op.op.type !== "update") return null;
            const { task_id, patch } = op.op;
            const before = tasks.find((t) => t.id === task_id);
            return (
              <label className="review-item" key={op.index}>
                <input type="checkbox" checked={selected.has(op.index)} onChange={() => toggle(op.index)} />
                <div className="ri-body">
                  <div className="ri-title">
                    <b>{titleOf(task_id)}</b>
                  </div>
                  {(Object.keys(patch) as (keyof TaskPatch)[]).map((f) => (
                    <div className="ri-diff" key={f}>
                      {FIELD_LABEL[f] ?? f}: {before ? fmt(f, before[f as keyof typeof before], titleOf) : "?"}
                      <span className="arrow">&rarr;</span>
                      <b>{fmt(f, patch[f], titleOf)}</b>
                    </div>
                  ))}
                  <div className="ri-reason">{op.reason}</div>
                </div>
              </label>
            );
          })}
        </div>
      )}

      {groups.completes.length > 0 && (
        <div className="review-group">
          <h3>Completed</h3>
          {groups.completes.map((op) => (
            <label className="review-item" key={op.index}>
              <input type="checkbox" checked={selected.has(op.index)} onChange={() => toggle(op.index)} />
              <div className="ri-body">
                <div className="ri-title">
                  <b>{op.op.type === "complete" ? titleOf(op.op.task_id) : ""}</b>
                </div>
                <div className="ri-reason">{op.reason}</div>
              </div>
            </label>
          ))}
        </div>
      )}

      {groups.notes.length > 0 && (
        <div className="review-group">
          <h3>Notes added</h3>
          {groups.notes.map((op) => (
            <label className="review-item" key={op.index}>
              <input type="checkbox" checked={selected.has(op.index)} onChange={() => toggle(op.index)} />
              <div className="ri-body">
                <div className="ri-title">
                  <b>{op.op.type === "add_note" ? titleOf(op.op.task_id) : ""}</b>
                </div>
                {op.op.type === "add_note" && <div className="ri-diff">{op.op.body}</div>}
                <div className="ri-reason">{op.reason}</div>
              </div>
            </label>
          ))}
        </div>
      )}

      {nothing && <p className="muted small">Nothing actionable was found in this context.</p>}

      <div className="sheet-foot" style={{ padding: "16px 0 0", marginTop: 8 }}>
        <span className="hint">Enter applies</span>
        <Button variant="ghost" onClick={discard} disabled={busy}>
          Discard
        </Button>
        <Button variant="primary" onClick={apply} disabled={busy || count === 0} autoFocus>
          {count === 0 ? "Nothing selected" : count === 1 ? "Apply 1 change" : `Apply ${count} changes`}
        </Button>
      </div>
    </>
  );
}
