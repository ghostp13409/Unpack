use crate::ai::ops::{Proposal, ProposedOp, ResolvedOp};
use crate::models::{new_id, now_iso, CaptureStatus, Change, ChangeKind, NewTask, Priority, Status, Task, TaskPatch};
use crate::repo::captures::{get_capture, get_change, insert_change, mark_undone, set_status};
use crate::repo::tasks::{add_note, delete_note, detach_children, get_task, insert_task, update_task};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("change not found")]
    NotFound,
    #[error("this change was already undone")]
    AlreadyUndone,
    #[error("task no longer exists")]
    TaskMissing,
    #[error("capture not found")]
    CaptureMissing,
    #[error("this capture was already applied")]
    AlreadyApplied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UndoOutcome {
    Done,
    NeedsConfirm { changed_fields: Vec<String> },
}

fn task_field(task: &Task, name: &str) -> Value {
    match name {
        "title" => json!(task.title),
        "description" => json!(task.description),
        "status" => json!(task.status),
        "priority" => json!(task.priority),
        "category" => json!(task.category),
        "due_date" => json!(task.due_date),
        "parent_id" => json!(task.parent_id),
        "completed_at" => json!(task.completed_at),
        _ => Value::Null,
    }
}

fn opt_str(v: &Value) -> Option<String> {
    v.as_str().map(|s| s.to_string())
}

/// Builds a patch that writes the given field values back onto a task.
fn patch_from_map(map: &Map<String, Value>) -> TaskPatch {
    let mut patch = TaskPatch::default();
    for (k, v) in map {
        match k.as_str() {
            "title" => patch.title = opt_str(v),
            "description" => patch.description = opt_str(v),
            "status" => patch.status = v.as_str().and_then(|s| Status::from_str(s).ok()),
            "priority" => patch.priority = v.as_str().and_then(|s| Priority::from_str(s).ok()),
            "category" => patch.category = Some(opt_str(v)),
            "due_date" => patch.due_date = Some(opt_str(v)),
            "parent_id" => patch.parent_id = Some(opt_str(v)),
            _ => {}
        }
    }
    patch
}

fn patched_fields(patch: &TaskPatch) -> Vec<(&'static str, Value)> {
    let mut out = Vec::new();
    if let Some(v) = &patch.title {
        out.push(("title", json!(v)));
    }
    if let Some(v) = &patch.description {
        out.push(("description", json!(v)));
    }
    if let Some(v) = &patch.status {
        out.push(("status", json!(v)));
    }
    if let Some(v) = &patch.priority {
        out.push(("priority", json!(v)));
    }
    if let Some(v) = &patch.category {
        out.push(("category", json!(v)));
    }
    if let Some(v) = &patch.due_date {
        out.push(("due_date", json!(v)));
    }
    if let Some(v) = &patch.parent_id {
        out.push(("parent_id", json!(v)));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn change(
    capture_id: &str,
    task_id: Option<String>,
    kind: ChangeKind,
    before: Value,
    after: Value,
    reason: String,
    evidence: Option<String>,
) -> Change {
    Change {
        id: new_id(),
        capture_id: capture_id.to_string(),
        task_id,
        kind,
        before,
        after,
        reason,
        evidence,
        applied: true,
        undone: false,
        created_at: now_iso(),
    }
}

/// Applies the selected operations (by `index`) in order inside one
/// transaction, records a Change per operation, and marks the capture
/// applied (or discarded when nothing was selected).
pub fn apply_proposal(
    conn: &Connection,
    capture_id: &str,
    proposal: &Proposal,
    selected: &[usize],
) -> Result<Vec<Change>, ApplyError> {
    let tx = conn.unchecked_transaction()?;
    let capture = get_capture(&tx, capture_id)?.ok_or(ApplyError::CaptureMissing)?;
    if capture.status != CaptureStatus::Proposed {
        return Err(ApplyError::AlreadyApplied);
    }
    let mut temp_ids: HashMap<String, String> = HashMap::new();
    let mut changes = Vec::new();

    for op in parents_first(proposal.ops.iter().filter(|o| selected.contains(&o.index)).collect()) {
        let mut reason = op.reason.clone();
        let evidence = op.evidence.clone();
        let c = match &op.op {
            ResolvedOp::Create { temp_id, new, parent_temp } => {
                let mut new: NewTask = new.clone();
                if let Some(pt) = parent_temp {
                    match temp_ids.get(pt) {
                        Some(real) => new.parent_id = Some(real.clone()),
                        None => reason.push_str(" (parent was skipped)"),
                    }
                }
                let task = insert_task(&tx, &new)?;
                if let Some(t) = temp_id {
                    temp_ids.insert(t.clone(), task.id.clone());
                }
                let after = json!({
                    "title": task.title, "priority": task.priority, "category": task.category,
                    "due_date": task.due_date, "parent_id": task.parent_id
                });
                change(capture_id, Some(task.id), ChangeKind::Create, json!({}), after, reason, evidence)
            }
            ResolvedOp::Update { task_id, patch } => {
                let current = get_task(&tx, task_id)?.ok_or(ApplyError::TaskMissing)?;
                let mut before = Map::new();
                let mut after = Map::new();
                for (name, value) in patched_fields(patch) {
                    before.insert(name.to_string(), task_field(&current, name));
                    after.insert(name.to_string(), value);
                }
                update_task(&tx, task_id, patch)?;
                change(
                    capture_id,
                    Some(task_id.clone()),
                    ChangeKind::Update,
                    Value::Object(before),
                    Value::Object(after),
                    reason,
                    evidence,
                )
            }
            ResolvedOp::Complete { task_id } => {
                let current = get_task(&tx, task_id)?.ok_or(ApplyError::TaskMissing)?;
                let before = json!({"status": current.status, "completed_at": current.completed_at});
                let updated = update_task(&tx, task_id, &TaskPatch { status: Some(Status::Done), ..Default::default() })?;
                let after = json!({"status": updated.status, "completed_at": updated.completed_at});
                change(capture_id, Some(task_id.clone()), ChangeKind::Complete, before, after, reason, evidence)
            }
            ResolvedOp::AddNote { task_id, body } => {
                let note = add_note(&tx, task_id, body, Some(capture_id))?;
                change(
                    capture_id,
                    Some(task_id.clone()),
                    ChangeKind::AddNote,
                    json!({}),
                    json!({"note_id": note.id, "body": note.body}),
                    reason,
                    evidence,
                )
            }
        };
        insert_change(&tx, &c)?;
        changes.push(c);
    }

    let status = if changes.is_empty() { CaptureStatus::Discarded } else { CaptureStatus::Applied };
    set_status(&tx, capture_id, status)?;
    tx.commit()?;
    Ok(changes)
}

/// Orders the selected operations so a `create` comes after the selected
/// `create` it names as its parent, whatever order the model listed them in.
/// Everything else keeps its original order.
fn parents_first(ops: Vec<&ProposedOp>) -> Vec<&ProposedOp> {
    let selected_temps: HashSet<&str> = ops
        .iter()
        .filter_map(|o| match &o.op {
            ResolvedOp::Create { temp_id: Some(t), .. } => Some(t.as_str()),
            _ => None,
        })
        .collect();
    let mut pending = ops;
    let mut done_temps: HashSet<&str> = HashSet::new();
    let mut out = Vec::with_capacity(pending.len());
    while !pending.is_empty() {
        let ready = pending.iter().position(|o| match &o.op {
            ResolvedOp::Create { parent_temp: Some(p), .. } => {
                !selected_temps.contains(p.as_str()) || done_temps.contains(p.as_str())
            }
            _ => true,
        });
        // A cycle among temp ids cannot be satisfied; take the first one as is.
        let op = pending.remove(ready.unwrap_or(0));
        if let ResolvedOp::Create { temp_id: Some(t), .. } = &op.op {
            done_temps.insert(t.as_str());
        }
        out.push(op);
    }
    out
}

/// Reverts one applied change. For field changes, refuses (NeedsConfirm)
/// when the task's current values no longer match what the change wrote,
/// unless `force` is set.
pub fn undo_change(conn: &Connection, change_id: &str, force: bool) -> Result<UndoOutcome, ApplyError> {
    let c = get_change(conn, change_id)?.ok_or(ApplyError::NotFound)?;
    if c.undone {
        return Err(ApplyError::AlreadyUndone);
    }
    let task_id = c.task_id.clone().ok_or(ApplyError::TaskMissing)?;
    match c.kind {
        ChangeKind::Create => {
            update_task(conn, &task_id, &TaskPatch { status: Some(Status::Dropped), ..Default::default() })?;
            detach_children(conn, &task_id)?;
        }
        ChangeKind::Update | ChangeKind::Complete => {
            let current = get_task(conn, &task_id)?.ok_or(ApplyError::TaskMissing)?;
            let after = c.after.as_object().cloned().unwrap_or_default();
            let changed: Vec<String> = after
                .iter()
                .filter(|(k, _)| k.as_str() != "completed_at")
                .filter(|(k, v)| task_field(&current, k) != **v)
                .map(|(k, _)| k.clone())
                .collect();
            if !changed.is_empty() && !force {
                return Ok(UndoOutcome::NeedsConfirm { changed_fields: changed });
            }
            let before = c.before.as_object().cloned().unwrap_or_default();
            update_task(conn, &task_id, &patch_from_map(&before))?;
        }
        ChangeKind::AddNote => {
            if let Some(note_id) = c.after["note_id"].as_str() {
                delete_note(conn, note_id)?;
            }
        }
    }
    mark_undone(conn, change_id)?;
    Ok(UndoOutcome::Done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ops::{Proposal, ProposedOp, ResolvedOp};
    use crate::db::open_memory;
    use crate::models::*;
    use crate::repo::captures::{get_capture, insert_capture, list_changes_for_capture, list_changes_for_task};
    use crate::repo::tasks::{get_task, insert_task, list_notes, list_tasks, update_task};

    fn new(title: &str) -> NewTask {
        NewTask {
            title: title.into(),
            description: String::new(),
            priority: Priority::Medium,
            category: None,
            due_date: None,
            parent_id: None,
        }
    }

    fn create_op(index: usize, temp: &str, title: &str, parent_temp: Option<&str>) -> ProposedOp {
        ProposedOp {
            index,
            op: ResolvedOp::Create {
                temp_id: Some(temp.into()),
                new: new(title),
                parent_temp: parent_temp.map(|s| s.to_string()),
            },
            reason: "r".into(),
            evidence: Some("quote".into()),
        }
    }

    fn proposal(ops: Vec<ProposedOp>) -> Proposal {
        Proposal { summary: "sum".into(), ops, warnings: vec![] }
    }

    #[test]
    fn apply_creates_parent_then_child_with_temp_link() {
        let conn = open_memory().unwrap();
        let cap = insert_capture(&conn, "m").unwrap();
        let p = proposal(vec![create_op(0, "t1", "Parent", None), create_op(1, "t2", "Child", Some("t1"))]);
        let changes = apply_proposal(&conn, &cap.id, &p, &[0, 1]).unwrap();
        assert_eq!(changes.len(), 2);
        let tasks = list_tasks(&conn, false).unwrap();
        let parent = tasks.iter().find(|t| t.title == "Parent").unwrap();
        let child = tasks.iter().find(|t| t.title == "Child").unwrap();
        assert_eq!(child.parent_id.as_deref(), Some(parent.id.as_str()));
        assert_eq!(changes[0].kind, ChangeKind::Create);
        assert_eq!(changes[0].task_id.as_deref(), Some(parent.id.as_str()));
        assert_eq!(changes[0].evidence.as_deref(), Some("quote"));
        assert!(changes.iter().all(|c| c.applied && !c.undone));
        assert_eq!(get_capture(&conn, &cap.id).unwrap().unwrap().status, CaptureStatus::Applied);
        assert_eq!(list_changes_for_capture(&conn, &cap.id).unwrap().len(), 2);
        assert_eq!(list_changes_for_task(&conn, &child.id).unwrap().len(), 1);
    }

    #[test]
    fn unselected_temp_parent_makes_child_top_level() {
        let conn = open_memory().unwrap();
        let cap = insert_capture(&conn, "m").unwrap();
        let p = proposal(vec![create_op(0, "t1", "Parent", None), create_op(1, "t2", "Child", Some("t1"))]);
        let changes = apply_proposal(&conn, &cap.id, &p, &[1]).unwrap();
        assert_eq!(changes.len(), 1);
        let tasks = list_tasks(&conn, false).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Child");
        assert!(tasks[0].parent_id.is_none());
        assert!(changes[0].reason.ends_with("(parent was skipped)"), "{}", changes[0].reason);
    }

    #[test]
    fn update_records_only_touched_fields() {
        let conn = open_memory().unwrap();
        let t = insert_task(&conn, &new("x")).unwrap();
        let cap = insert_capture(&conn, "m").unwrap();
        let op = ProposedOp {
            index: 0,
            op: ResolvedOp::Update {
                task_id: t.id.clone(),
                patch: TaskPatch {
                    priority: Some(Priority::High),
                    due_date: Some(Some("2026-11-05".into())),
                    ..Default::default()
                },
            },
            reason: "r".into(),
            evidence: None,
        };
        let changes = apply_proposal(&conn, &cap.id, &proposal(vec![op]), &[0]).unwrap();
        let c = &changes[0];
        assert_eq!(c.kind, ChangeKind::Update);
        assert_eq!(c.before, serde_json::json!({"priority": "medium", "due_date": null}));
        assert_eq!(c.after, serde_json::json!({"priority": "high", "due_date": "2026-11-05"}));
        let t2 = get_task(&conn, &t.id).unwrap().unwrap();
        assert_eq!(t2.priority, Priority::High);
        assert_eq!(t2.due_date.as_deref(), Some("2026-11-05"));
    }

    #[test]
    fn complete_sets_done_and_undo_restores() {
        let conn = open_memory().unwrap();
        let t = insert_task(&conn, &new("x")).unwrap();
        let cap = insert_capture(&conn, "m").unwrap();
        let op = ProposedOp {
            index: 0,
            op: ResolvedOp::Complete { task_id: t.id.clone() },
            reason: "r".into(),
            evidence: None,
        };
        let changes = apply_proposal(&conn, &cap.id, &proposal(vec![op]), &[0]).unwrap();
        assert_eq!(get_task(&conn, &t.id).unwrap().unwrap().status, Status::Done);
        assert_eq!(changes[0].kind, ChangeKind::Complete);
        assert_eq!(changes[0].before["status"], "todo");
        assert_eq!(changes[0].after["status"], "done");
        let outcome = undo_change(&conn, &changes[0].id, false).unwrap();
        assert_eq!(outcome, UndoOutcome::Done);
        let t2 = get_task(&conn, &t.id).unwrap().unwrap();
        assert_eq!(t2.status, Status::Todo);
        assert!(t2.completed_at.is_none());
    }

    #[test]
    fn undo_create_marks_dropped_and_hides_from_list() {
        let conn = open_memory().unwrap();
        let cap = insert_capture(&conn, "m").unwrap();
        let changes =
            apply_proposal(&conn, &cap.id, &proposal(vec![create_op(0, "t1", "New", None)]), &[0]).unwrap();
        assert_eq!(undo_change(&conn, &changes[0].id, false).unwrap(), UndoOutcome::Done);
        assert!(list_tasks(&conn, true).unwrap().is_empty());
        let id = changes[0].task_id.clone().unwrap();
        assert_eq!(get_task(&conn, &id).unwrap().unwrap().status, Status::Dropped);
    }

    #[test]
    fn undo_update_after_manual_edit_needs_confirm_then_force_overwrites() {
        let conn = open_memory().unwrap();
        let t = insert_task(&conn, &new("x")).unwrap();
        let cap = insert_capture(&conn, "m").unwrap();
        let op = ProposedOp {
            index: 0,
            op: ResolvedOp::Update {
                task_id: t.id.clone(),
                patch: TaskPatch { priority: Some(Priority::High), ..Default::default() },
            },
            reason: "r".into(),
            evidence: None,
        };
        let changes = apply_proposal(&conn, &cap.id, &proposal(vec![op]), &[0]).unwrap();
        update_task(&conn, &t.id, &TaskPatch { priority: Some(Priority::Urgent), ..Default::default() }).unwrap();
        let outcome = undo_change(&conn, &changes[0].id, false).unwrap();
        assert_eq!(outcome, UndoOutcome::NeedsConfirm { changed_fields: vec!["priority".into()] });
        assert_eq!(get_task(&conn, &t.id).unwrap().unwrap().priority, Priority::Urgent, "nothing changed yet");
        assert_eq!(undo_change(&conn, &changes[0].id, true).unwrap(), UndoOutcome::Done);
        assert_eq!(get_task(&conn, &t.id).unwrap().unwrap().priority, Priority::Medium);
    }

    #[test]
    fn undo_note_deletes_it() {
        let conn = open_memory().unwrap();
        let t = insert_task(&conn, &new("x")).unwrap();
        let cap = insert_capture(&conn, "m").unwrap();
        let op = ProposedOp {
            index: 0,
            op: ResolvedOp::AddNote { task_id: t.id.clone(), body: "info".into() },
            reason: "r".into(),
            evidence: None,
        };
        let changes = apply_proposal(&conn, &cap.id, &proposal(vec![op]), &[0]).unwrap();
        let notes = list_notes(&conn, &t.id).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].source_capture_id.as_deref(), Some(cap.id.as_str()));
        assert_eq!(changes[0].after["note_id"], notes[0].id);
        undo_change(&conn, &changes[0].id, false).unwrap();
        assert!(list_notes(&conn, &t.id).unwrap().is_empty());
    }

    #[test]
    fn undo_twice_is_error() {
        let conn = open_memory().unwrap();
        let cap = insert_capture(&conn, "m").unwrap();
        let changes =
            apply_proposal(&conn, &cap.id, &proposal(vec![create_op(0, "t1", "New", None)]), &[0]).unwrap();
        undo_change(&conn, &changes[0].id, false).unwrap();
        assert!(matches!(undo_change(&conn, &changes[0].id, false), Err(ApplyError::AlreadyUndone)));
        assert!(matches!(undo_change(&conn, "missing", false), Err(ApplyError::NotFound)));
    }

    #[test]
    fn nothing_selected_marks_capture_discarded() {
        let conn = open_memory().unwrap();
        let cap = insert_capture(&conn, "m").unwrap();
        let changes =
            apply_proposal(&conn, &cap.id, &proposal(vec![create_op(0, "t1", "New", None)]), &[]).unwrap();
        assert!(changes.is_empty());
        assert_eq!(get_capture(&conn, &cap.id).unwrap().unwrap().status, CaptureStatus::Discarded);
    }


    #[test]
    fn applying_the_same_capture_twice_is_rejected() {
        let conn = open_memory().unwrap();
        let cap = insert_capture(&conn, "m").unwrap();
        let p = proposal(vec![create_op(0, "t1", "Once", None)]);
        apply_proposal(&conn, &cap.id, &p, &[0]).unwrap();
        let err = apply_proposal(&conn, &cap.id, &p, &[0]).unwrap_err();
        assert!(matches!(err, ApplyError::AlreadyApplied), "{err:?}");
        assert_eq!(list_tasks(&conn, true).unwrap().len(), 1, "no duplicate task");
        assert_eq!(list_changes_for_capture(&conn, &cap.id).unwrap().len(), 1);
    }

    #[test]
    fn child_listed_before_its_parent_still_nests() {
        let conn = open_memory().unwrap();
        let cap = insert_capture(&conn, "m").unwrap();
        let p = proposal(vec![create_op(0, "t2", "Child", Some("t1")), create_op(1, "t1", "Parent", None)]);
        let changes = apply_proposal(&conn, &cap.id, &p, &[0, 1]).unwrap();
        let tasks = list_tasks(&conn, false).unwrap();
        let parent = tasks.iter().find(|t| t.title == "Parent").unwrap();
        let child = tasks.iter().find(|t| t.title == "Child").unwrap();
        assert_eq!(child.parent_id.as_deref(), Some(parent.id.as_str()));
        let child_change = changes.iter().find(|c| c.task_id.as_deref() == Some(child.id.as_str())).unwrap();
        assert!(!child_change.reason.contains("parent was skipped"), "{}", child_change.reason);
    }

    #[test]
    fn undo_create_detaches_its_subtasks() {
        let conn = open_memory().unwrap();
        let cap = insert_capture(&conn, "m").unwrap();
        let changes = apply_proposal(&conn, &cap.id, &proposal(vec![create_op(0, "t1", "Parent", None)]), &[0]).unwrap();
        let parent_id = changes[0].task_id.clone().unwrap();
        let child = insert_task(&conn, &NewTask { parent_id: Some(parent_id.clone()), ..new("Child") }).unwrap();
        assert_eq!(undo_change(&conn, &changes[0].id, false).unwrap(), UndoOutcome::Done);
        let child = get_task(&conn, &child.id).unwrap().unwrap();
        assert!(child.parent_id.is_none(), "child should be detached from the dropped parent");
        assert_eq!(child.status, Status::Todo);
    }
}
