use crate::models::{new_id, now_iso, NewTask, Note, Priority, Status, Task, TaskPatch};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::str::FromStr;

pub type RepoError = rusqlite::Error;
pub type Result<T> = std::result::Result<T, RepoError>;

const TASK_COLS: &str = "id, title, description, status, priority, category, due_date, parent_id, pinned, snoozed_until, created_at, updated_at, completed_at";

fn parse_err(msg: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, msg.into())
}

pub(crate) fn row_to_task(row: &Row) -> rusqlite::Result<Task> {
    let status: String = row.get(3)?;
    let priority: String = row.get(4)?;
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        status: Status::from_str(&status).map_err(parse_err)?,
        priority: Priority::from_str(&priority).map_err(parse_err)?,
        category: row.get(5)?,
        due_date: row.get(6)?,
        parent_id: row.get(7)?,
        pinned: row.get::<_, i64>(8)? != 0,
        snoozed_until: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
        completed_at: row.get(12)?,
    })
}

pub fn insert_task(conn: &Connection, new: &NewTask) -> Result<Task> {
    let id = new_id();
    let now = now_iso();
    conn.execute(
        "INSERT INTO tasks (id, title, description, status, priority, category, due_date, parent_id, pinned, snoozed_until, created_at, updated_at, completed_at)
         VALUES (?1, ?2, ?3, 'todo', ?4, ?5, ?6, ?7, 0, NULL, ?8, ?8, NULL)",
        params![id, new.title, new.description, new.priority.as_str(), new.category, new.due_date, new.parent_id, now],
    )?;
    get_task(conn, &id).map(|t| t.expect("just inserted"))
}

pub fn get_task(conn: &Connection, id: &str) -> Result<Option<Task>> {
    conn.query_row(&format!("SELECT {TASK_COLS} FROM tasks WHERE id = ?1"), [id], row_to_task)
        .optional()
}

pub fn list_tasks(conn: &Connection, include_done: bool) -> Result<Vec<Task>> {
    let sql = if include_done {
        format!("SELECT {TASK_COLS} FROM tasks WHERE status != 'dropped' ORDER BY created_at, id")
    } else {
        format!("SELECT {TASK_COLS} FROM tasks WHERE status IN ('todo','in_progress') ORDER BY created_at, id")
    };
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_task)?;
    rows.collect()
}

pub fn update_task(conn: &Connection, id: &str, patch: &TaskPatch) -> Result<Task> {
    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut push = |col: &str, v: Box<dyn rusqlite::ToSql>| {
        sets.push(format!("{col} = ?{}", values.len() + 1));
        values.push(v);
    };
    if let Some(v) = &patch.title {
        push("title", Box::new(v.clone()));
    }
    if let Some(v) = &patch.description {
        push("description", Box::new(v.clone()));
    }
    if let Some(v) = &patch.status {
        push("status", Box::new(v.as_str()));
        let completed: Option<String> = if *v == Status::Done { Some(now_iso()) } else { None };
        push("completed_at", Box::new(completed));
    }
    if let Some(v) = &patch.priority {
        push("priority", Box::new(v.as_str()));
    }
    if let Some(v) = &patch.category {
        push("category", Box::new(v.clone()));
    }
    if let Some(v) = &patch.due_date {
        push("due_date", Box::new(v.clone()));
    }
    if let Some(v) = &patch.parent_id {
        push("parent_id", Box::new(v.clone()));
    }
    if let Some(v) = &patch.pinned {
        push("pinned", Box::new(*v as i64));
    }
    if let Some(v) = &patch.snoozed_until {
        push("snoozed_until", Box::new(v.clone()));
    }
    push("updated_at", Box::new(now_iso()));
    let n = values.len() + 1;
    values.push(Box::new(id.to_string()));
    let sql = format!("UPDATE tasks SET {} WHERE id = ?{n}", sets.join(", "));
    let refs: Vec<&dyn rusqlite::ToSql> = values.iter().map(|b| b.as_ref()).collect();
    let changed = conn.execute(&sql, refs.as_slice())?;
    if changed == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    get_task(conn, id).map(|t| t.expect("exists"))
}

fn row_to_note(row: &Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        task_id: row.get(1)?,
        body: row.get(2)?,
        source_capture_id: row.get(3)?,
        created_at: row.get(4)?,
    })
}

pub fn add_note(conn: &Connection, task_id: &str, body: &str, source_capture_id: Option<&str>) -> Result<Note> {
    let id = new_id();
    let now = now_iso();
    conn.execute(
        "INSERT INTO notes (id, task_id, body, source_capture_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, task_id, body, source_capture_id, now],
    )?;
    Ok(Note { id, task_id: task_id.to_string(), body: body.to_string(), source_capture_id: source_capture_id.map(|s| s.to_string()), created_at: now })
}

pub fn list_notes(conn: &Connection, task_id: &str) -> Result<Vec<Note>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, body, source_capture_id, created_at FROM notes WHERE task_id = ?1 ORDER BY created_at, rowid",
    )?;
    let rows = stmt.query_map([task_id], row_to_note)?;
    rows.collect()
}

/// Moves every sub-task of `parent_id` to top level (used when the parent is dropped).
pub fn detach_children(conn: &Connection, parent_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE tasks SET parent_id = NULL, updated_at = ?2 WHERE parent_id = ?1",
        rusqlite::params![parent_id, crate::models::now_iso()],
    )?;
    Ok(())
}

pub fn delete_note(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM notes WHERE id = ?1", [id])?;
    Ok(())
}

pub fn list_categories(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT category FROM tasks WHERE status != 'dropped' AND category IS NOT NULL AND category != '' ORDER BY category COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;
    use crate::models::*;

    fn new(title: &str) -> NewTask {
        NewTask {
            title: title.to_string(),
            description: String::new(),
            priority: Priority::Medium,
            category: None,
            due_date: None,
            parent_id: None,
        }
    }

    #[test]
    fn insert_then_get_roundtrips() {
        let conn = open_memory().unwrap();
        let mut n = new("Write report");
        n.description = "for Alex".into();
        n.priority = Priority::High;
        n.category = Some("Platform".into());
        n.due_date = Some("2026-11-05".into());
        let t = insert_task(&conn, &n).unwrap();
        let got = get_task(&conn, &t.id).unwrap().unwrap();
        assert_eq!(got, t);
        assert_eq!(got.title, "Write report");
        assert_eq!(got.description, "for Alex");
        assert_eq!(got.status, Status::Todo);
        assert_eq!(got.priority, Priority::High);
        assert_eq!(got.category.as_deref(), Some("Platform"));
        assert_eq!(got.due_date.as_deref(), Some("2026-11-05"));
        assert!(!got.created_at.is_empty() && got.created_at == got.updated_at);
        assert!(!got.pinned);
        assert!(get_task(&conn, "nope").unwrap().is_none());
    }

    #[test]
    fn list_excludes_done_unless_asked() {
        let conn = open_memory().unwrap();
        let a = insert_task(&conn, &new("a")).unwrap();
        insert_task(&conn, &new("b")).unwrap();
        let patch = TaskPatch { status: Some(Status::Done), ..Default::default() };
        update_task(&conn, &a.id, &patch).unwrap();
        assert_eq!(list_tasks(&conn, false).unwrap().len(), 1);
        assert_eq!(list_tasks(&conn, true).unwrap().len(), 2);
    }

    #[test]
    fn list_never_returns_dropped() {
        let conn = open_memory().unwrap();
        let a = insert_task(&conn, &new("a")).unwrap();
        update_task(&conn, &a.id, &TaskPatch { status: Some(Status::Dropped), ..Default::default() }).unwrap();
        assert!(list_tasks(&conn, true).unwrap().is_empty());
        assert!(get_task(&conn, &a.id).unwrap().is_some(), "still fetchable by id");
    }

    #[test]
    fn update_sets_completed_at_on_done_and_clears_on_reopen() {
        let conn = open_memory().unwrap();
        let a = insert_task(&conn, &new("a")).unwrap();
        let done = update_task(&conn, &a.id, &TaskPatch { status: Some(Status::Done), ..Default::default() }).unwrap();
        assert_eq!(done.status, Status::Done);
        assert!(done.completed_at.is_some());
        let reopened = update_task(&conn, &a.id, &TaskPatch { status: Some(Status::Todo), ..Default::default() }).unwrap();
        assert!(reopened.completed_at.is_none());
        assert!(reopened.updated_at >= a.updated_at);
    }

    #[test]
    fn patch_can_clear_due_date() {
        let conn = open_memory().unwrap();
        let mut n = new("a");
        n.due_date = Some("2026-11-05".into());
        let a = insert_task(&conn, &n).unwrap();
        let patch: TaskPatch = serde_json::from_str(r#"{"due_date": null, "title": "b"}"#).unwrap();
        assert_eq!(patch.due_date, Some(None));
        assert_eq!(patch.category, None, "absent field stays untouched");
        let t = update_task(&conn, &a.id, &patch).unwrap();
        assert_eq!(t.due_date, None);
        assert_eq!(t.title, "b");
    }

    #[test]
    fn notes_roundtrip_and_delete() {
        let conn = open_memory().unwrap();
        let a = insert_task(&conn, &new("a")).unwrap();
        let n1 = add_note(&conn, &a.id, "first", None).unwrap();
        let n2 = add_note(&conn, &a.id, "second", Some("cap1")).unwrap();
        let notes = list_notes(&conn, &a.id).unwrap();
        assert_eq!(notes, vec![n1.clone(), n2.clone()]);
        assert_eq!(n2.source_capture_id.as_deref(), Some("cap1"));
        delete_note(&conn, &n1.id).unwrap();
        assert_eq!(list_notes(&conn, &a.id).unwrap(), vec![n2]);
    }

    #[test]
    fn categories_are_distinct_sorted() {
        let conn = open_memory().unwrap();
        for c in ["Observability", "Admin", "Observability", ""] {
            let mut n = new("x");
            n.category = if c.is_empty() { None } else { Some(c.into()) };
            insert_task(&conn, &n).unwrap();
        }
        let mut d = new("dropped");
        d.category = Some("Gone".into());
        let d = insert_task(&conn, &d).unwrap();
        update_task(&conn, &d.id, &TaskPatch { status: Some(Status::Dropped), ..Default::default() }).unwrap();
        assert_eq!(list_categories(&conn).unwrap(), vec!["Admin".to_string(), "Observability".to_string()]);
    }
}
