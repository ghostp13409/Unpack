import { ChevronRight, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "../components/Button";
import { EmptyState } from "../components/EmptyState";
import * as api from "../lib/api";
import { formatDateTime } from "../lib/dates";
import type { CaptureSummary, Change } from "../lib/types";
import { PRIORITY_LABEL } from "../lib/types";
import { useStore } from "../store/useStore";

function describe(c: Change, titleOf: (id: string | null) => string): string {
  const t = titleOf(c.task_id);
  switch (c.kind) {
    case "create":
      return `Created "${String(c.after.title ?? t)}"`;
    case "complete":
      return `Completed "${t}"`;
    case "add_note":
      return `Note on "${t}": ${String(c.after.body ?? "")}`;
    case "update": {
      const parts = Object.keys(c.after).map((k) => {
        const f = (v: unknown) =>
          v === null || v === undefined || v === "" ? "none" : k === "priority" ? PRIORITY_LABEL[v as keyof typeof PRIORITY_LABEL] ?? String(v) : String(v);
        return `${k.replace("_", " ")} ${f(c.before[k])} → ${f(c.after[k])}`;
      });
      return `Updated "${t}": ${parts.join(", ")}`;
    }
  }
}

function ChangeLine({ change, titleOf, onUndone }: { change: Change; titleOf: (id: string | null) => string; onUndone: () => void }) {
  const [confirm, setConfirm] = useState<string[] | null>(null);
  const [busy, setBusy] = useState(false);
  const setError = useStore((s) => s.setError);

  const undo = async (force: boolean) => {
    setBusy(true);
    try {
      const out = await api.undoChange(change.id, force);
      if (out.type === "needs_confirm") setConfirm(out.changed_fields);
      else {
        setConfirm(null);
        onUndone();
      }
    } catch (e) {
      setError(api.errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={`change-line ${change.undone ? "undone" : ""}`}>
      <span className="cl-text" title={change.reason}>
        {describe(change, titleOf)}
      </span>
      {confirm ? (
        <span className="confirm-inline">
          This task changed since ({confirm.join(", ")}). Overwrite?
          <Button size="sm" onClick={() => undo(true)} disabled={busy}>
            Yes
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setConfirm(null)}>
            No
          </Button>
        </span>
      ) : (
        !change.undone && (
          <Button size="sm" variant="ghost" onClick={() => undo(false)} disabled={busy}>
            Undo
          </Button>
        )
      )}
    </div>
  );
}

function CaptureCard({ item, titleOf, onChanged }: { item: CaptureSummary; titleOf: (id: string | null) => string; onChanged: () => void }) {
  const [open, setOpen] = useState(false);
  const { capture, inputs, changes } = item;
  const files = inputs.filter((i) => i.kind === "file").length;
  const pasted = inputs.some((i) => i.kind === "paste");
  const meta = [
    files > 0 ? (files === 1 ? "1 file" : `${files} files`) : null,
    pasted ? "pasted text" : null,
    changes.length === 1 ? "1 change" : `${changes.length} changes`,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <div className="capture-card">
      <div className="capture-head" onClick={() => setOpen((v) => !v)}>
        <ChevronRight size={14} style={{ transform: open ? "rotate(90deg)" : undefined, transition: "transform 150ms" }} />
        <span className="ch-date">{formatDateTime(capture.created_at)}</span>
        <span className="ch-meta">{meta}</span>
        <span className={`status-pill ${capture.status}`}>{capture.status}</span>
      </div>
      {open && (
        <div className="capture-body">
          {inputs.length > 0 && (
            <div className="xs muted" style={{ marginTop: 8 }}>
              {inputs.map((i) => `${i.filename ?? "Pasted text"} (${i.char_count.toLocaleString()} chars)`).join(" · ")}
            </div>
          )}
          {capture.summary && <p className="cb-summary">{capture.summary}</p>}
          {changes.map((c) => (
            <ChangeLine key={c.id} change={c} titleOf={titleOf} onUndone={onChanged} />
          ))}
          {changes.length === 0 && <div className="muted xs">No changes were applied.</div>}
        </div>
      )}
    </div>
  );
}

export function ActivityView() {
  const captures = useStore((s) => s.captures);
  const refreshActivity = useStore((s) => s.refreshActivity);
  const loadAll = useStore((s) => s.loadAll);
  const tasks = useStore((s) => s.tasks);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    refreshActivity().finally(() => setLoading(false));
  }, [refreshActivity]);

  const titleOf = (id: string | null) => (id ? tasks.find((t) => t.id === id)?.title ?? "a task" : "a task");
  const onChanged = async () => {
    await Promise.all([refreshActivity(), loadAll()]);
  };

  return (
    <div className="page">
      <div className="page-inner">
        <header className="page-header">
          <div>
            <h1>Activity</h1>
            <div className="page-sub">What each capture changed. Undo anything here.</div>
          </div>
        </header>
        {loading ? (
          <div className="notice">
            <Loader2 size={14} className="spin" /> Loading
          </div>
        ) : captures.length === 0 ? (
          <EmptyState message="No captures yet." />
        ) : (
          captures.map((c) => <CaptureCard key={c.capture.id} item={c} titleOf={titleOf} onChanged={onChanged} />)
        )}
      </div>
    </div>
  );
}
