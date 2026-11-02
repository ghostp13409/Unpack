import { Clock, Pin, Plus, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button } from "../components/Button";
import { Checkbox } from "../components/Checkbox";
import * as api from "../lib/api";
import { addDays, formatDateOnly, formatDateTime, todayISO } from "../lib/dates";
import type { Priority, Status, TaskDetail as Detail } from "../lib/types";
import { PRIORITIES, PRIORITY_LABEL, STATUS_LABEL } from "../lib/types";
import { useStore } from "../store/useStore";

export function TaskDetail({ id }: { id: string }) {
  const tasks = useStore((s) => s.tasks);
  const selectTask = useStore((s) => s.selectTask);
  const patchTask = useStore((s) => s.patchTask);
  const loadAll = useStore((s) => s.loadAll);
  const setError = useStore((s) => s.setError);

  const [detail, setDetail] = useState<Detail | null>(null);
  const [categories, setCategories] = useState<string[]>([]);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [newSub, setNewSub] = useState("");
  const [newNote, setNewNote] = useState("");
  const [snoozeOpen, setSnoozeOpen] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [d, cats] = await Promise.all([api.getTaskDetail(id), api.listCategories()]);
      setDetail(d);
      setCategories(cats);
      setTitle(d.task.title);
      setDescription(d.task.description);
    } catch (e) {
      setError(api.errorMessage(e));
      selectTask(null);
    }
  }, [id, selectTask, setError]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Keep the panel in sync with store changes to this task (e.g. checkbox in the list).
  const storeTask = tasks.find((t) => t.id === id);
  useEffect(() => {
    if (storeTask && detail && storeTask.updated_at !== detail.task.updated_at) void refresh();
  }, [storeTask, detail, refresh]);

  if (!detail) return <aside className="detail" />;
  const { task, notes, children, sources } = detail;

  const save = async (patch: Parameters<typeof patchTask>[1]) => {
    const updated = await patchTask(id, patch);
    if (updated) await refresh();
  };

  const addSub = async () => {
    const t = newSub.trim();
    if (!t) return;
    try {
      await api.createTask({ title: t, parent_id: id, category: task.category, priority: task.priority });
      setNewSub("");
      await loadAll();
      await refresh();
    } catch (e) {
      setError(api.errorMessage(e));
    }
  };

  const addNote = async () => {
    const b = newNote.trim();
    if (!b) return;
    try {
      await api.addNote(id, b);
      setNewNote("");
      await refresh();
    } catch (e) {
      setError(api.errorMessage(e));
    }
  };

  const topLevelOthers = tasks.filter((t) => t.parent_id === null && t.id !== id && t.status !== "dropped");
  const snoozed = task.snoozed_until && task.snoozed_until > todayISO();

  return (
    <aside className="detail" aria-label="Task details">
      <div className="detail-head">
        <select
          className="select select-sm"
          style={{ width: "auto" }}
          value={task.status}
          onChange={(e) => save({ status: e.target.value as Status })}
          aria-label="Status"
        >
          {(["todo", "in_progress", "done"] as Status[]).map((s) => (
            <option key={s} value={s}>
              {STATUS_LABEL[s]}
            </option>
          ))}
        </select>
        <div style={{ flex: 1 }} />
        <button className="icon-btn" onClick={() => selectTask(null)} aria-label="Close" title="Close (Esc)">
          <X size={16} />
        </button>
      </div>
      <div className="detail-body">
        <input
          id="task-title"
          className={`detail-title ${task.status === "done" ? "done" : ""}`}
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onBlur={() => title.trim() && title !== task.title && save({ title: title.trim() })}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
          aria-label="Title"
        />

        <div className="props">
          <label>Priority</label>
          <select className="select select-sm" value={task.priority} onChange={(e) => save({ priority: e.target.value as Priority })}>
            {PRIORITIES.map((p) => (
              <option key={p} value={p}>
                {PRIORITY_LABEL[p]}
              </option>
            ))}
          </select>

          <label>Category</label>
          <input
            className="input input-sm"
            list="categories"
            defaultValue={task.category ?? ""}
            key={`cat-${task.id}-${task.category ?? ""}`}
            placeholder="None"
            onBlur={(e) => {
              const v = e.target.value.trim();
              if (v !== (task.category ?? "")) save({ category: v || null });
            }}
          />
          <datalist id="categories">
            {categories.map((c) => (
              <option key={c} value={c} />
            ))}
          </datalist>

          <label>Due</label>
          <input
            className="input input-sm"
            type="date"
            value={task.due_date ?? ""}
            onChange={(e) => save({ due_date: e.target.value || null })}
          />

          <label>Parent</label>
          <select
            className="select select-sm"
            value={task.parent_id ?? ""}
            onChange={(e) => save({ parent_id: e.target.value || null })}
            disabled={children.length > 0}
            title={children.length > 0 ? "A task with sub-tasks cannot be nested" : undefined}
          >
            <option value="">None</option>
            {topLevelOthers.map((t) => (
              <option key={t.id} value={t.id}>
                {t.title}
              </option>
            ))}
          </select>
        </div>

        <textarea
          className="textarea"
          placeholder="Add a description"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          onBlur={() => description !== task.description && save({ description })}
          rows={Math.min(12, Math.max(3, description.split("\n").length + 1))}
        />

        {task.parent_id === null && (
          <section className="section">
            <div className="section-head">
              <h3>Sub-tasks</h3>
              {children.length > 0 && (
                <span className="xs faint">
                  {children.filter((c) => c.status === "done").length}/{children.length}
                </span>
              )}
            </div>
            {children.map((c) => (
              <div key={c.id} className={`sub-item ${c.status === "done" ? "done" : ""}`}>
                <Checkbox
                  checked={c.status === "done"}
                  onChange={async () => {
                    await patchTask(c.id, { status: c.status === "done" ? "todo" : "done" });
                    await refresh();
                  }}
                />
                <span style={{ flex: 1, cursor: "pointer" }} onClick={() => selectTask(c.id)}>
                  {c.title}
                </span>
              </div>
            ))}
            <div className="inline-add">
              <input
                className="input input-sm"
                placeholder="Add a sub-task"
                value={newSub}
                onChange={(e) => setNewSub(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && addSub()}
              />
              <button className="icon-btn" onClick={addSub} aria-label="Add sub-task">
                <Plus size={15} />
              </button>
            </div>
          </section>
        )}

        <section className="section">
          <div className="section-head">
            <h3>Notes</h3>
          </div>
          {notes.map((n) => (
            <div key={n.id} className="note">
              <span className="note-date">{formatDateTime(n.created_at)}</span>
              {n.body}
              <button
                className="icon-btn"
                aria-label="Delete note"
                onClick={async () => {
                  await api.deleteNote(n.id);
                  await refresh();
                }}
              >
                <Trash2 size={13} />
              </button>
            </div>
          ))}
          <div className="inline-add">
            <input
              className="input input-sm"
              placeholder="Add a note"
              value={newNote}
              onChange={(e) => setNewNote(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addNote()}
            />
            <button className="icon-btn" onClick={addNote} aria-label="Add note">
              <Plus size={15} />
            </button>
          </div>
        </section>

        {sources.length > 0 && (
          <section className="section">
            <div className="section-head">
              <h3>Sources</h3>
            </div>
            {sources.map((s, i) => (
              <div className="source" key={`${s.capture_id}-${i}`} style={s.undone ? { opacity: 0.5 } : undefined}>
                <div className="src-head">
                  <span className="src-kind">{s.kind.replace("_", " ")}</span>
                  <span>{s.captured_at ? formatDateOnly(s.captured_at) : ""}</span>
                  <span>{s.filenames.join(", ")}</span>
                </div>
                <div className="small">{s.reason}</div>
                {s.evidence && <blockquote className="small">{s.evidence}</blockquote>}
              </div>
            ))}
          </section>
        )}
      </div>
      <div className="detail-foot">
        <Button size="sm" variant={task.pinned ? "primary" : "default"} onClick={() => save({ pinned: !task.pinned })}>
          <Pin size={13} /> {task.pinned ? "Pinned" : "Pin"}
        </Button>
        <Button size="sm" onClick={() => setSnoozeOpen((v) => !v)}>
          <Clock size={13} /> {snoozed ? `Snoozed until ${formatDateOnly(task.snoozed_until!)}` : "Snooze"}
        </Button>
        {snoozeOpen && (
          <div className="menu" onMouseLeave={() => setSnoozeOpen(false)}>
            {[1, 3, 7].map((d) => (
              <button
                key={d}
                onClick={() => {
                  save({ snoozed_until: addDays(todayISO(), d) });
                  setSnoozeOpen(false);
                }}
              >
                {d === 1 ? "1 day" : `${d} days`}
              </button>
            ))}
            {snoozed && (
              <button
                onClick={() => {
                  save({ snoozed_until: null });
                  setSnoozeOpen(false);
                }}
              >
                Clear snooze
              </button>
            )}
          </div>
        )}
      </div>
    </aside>
  );
}
