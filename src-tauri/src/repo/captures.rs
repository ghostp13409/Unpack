use crate::ai::ops::Proposal;
use crate::models::{new_id, now_iso, Capture, CaptureInput, CaptureStatus, Change, ChangeKind, InputKind};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::str::FromStr;

pub type Result<T> = std::result::Result<T, rusqlite::Error>;

fn parse_err(msg: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, msg.into())
}

const CAPTURE_COLS: &str = "id, created_at, summary, model, input_tokens, output_tokens, status, proposal_json";

fn row_to_capture(row: &Row) -> rusqlite::Result<Capture> {
    let status: String = row.get(6)?;
    let proposal_json: Option<String> = row.get(7)?;
    let proposal = match proposal_json {
        Some(j) => Some(serde_json::from_str::<Proposal>(&j).map_err(|e| parse_err(e.to_string()))?),
        None => None,
    };
    Ok(Capture {
        id: row.get(0)?,
        created_at: row.get(1)?,
        summary: row.get(2)?,
        model: row.get(3)?,
        input_tokens: row.get(4)?,
        output_tokens: row.get(5)?,
        status: CaptureStatus::from_str(&status).map_err(parse_err)?,
        proposal,
    })
}

pub fn insert_capture(conn: &Connection, model: &str) -> Result<Capture> {
    let id = new_id();
    let now = now_iso();
    conn.execute(
        "INSERT INTO captures (id, created_at, model, status) VALUES (?1, ?2, ?3, 'proposed')",
        params![id, now, model],
    )?;
    Ok(Capture {
        id,
        created_at: now,
        summary: None,
        model: model.to_string(),
        input_tokens: 0,
        output_tokens: 0,
        status: CaptureStatus::Proposed,
        proposal: None,
    })
}

pub fn insert_input(conn: &Connection, capture_id: &str, kind: InputKind, filename: Option<&str>, text: &str) -> Result<CaptureInput> {
    let id = new_id();
    let char_count = text.chars().count() as i64;
    conn.execute(
        "INSERT INTO capture_inputs (id, capture_id, kind, filename, extracted_text, char_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, capture_id, kind.as_str(), filename, text, char_count],
    )?;
    Ok(CaptureInput {
        id,
        capture_id: capture_id.to_string(),
        kind,
        filename: filename.map(|s| s.to_string()),
        extracted_text: text.to_string(),
        char_count,
    })
}

pub fn set_proposal(conn: &Connection, id: &str, proposal: &Proposal, usage: (i64, i64)) -> Result<()> {
    let json = serde_json::to_string(proposal).map_err(|e| parse_err(e.to_string()))?;
    conn.execute(
        "UPDATE captures SET summary = ?2, proposal_json = ?3, input_tokens = ?4, output_tokens = ?5 WHERE id = ?1",
        params![id, proposal.summary, json, usage.0, usage.1],
    )?;
    Ok(())
}

pub fn set_status(conn: &Connection, id: &str, status: CaptureStatus) -> Result<()> {
    conn.execute("UPDATE captures SET status = ?2 WHERE id = ?1", params![id, status.as_str()])?;
    Ok(())
}

pub fn get_capture(conn: &Connection, id: &str) -> Result<Option<Capture>> {
    conn.query_row(&format!("SELECT {CAPTURE_COLS} FROM captures WHERE id = ?1"), [id], row_to_capture)
        .optional()
}

pub fn list_captures(conn: &Connection, limit: usize) -> Result<Vec<Capture>> {
    let mut stmt = conn.prepare(&format!("SELECT {CAPTURE_COLS} FROM captures ORDER BY created_at DESC, rowid DESC LIMIT ?1"))?;
    let rows = stmt.query_map([limit as i64], row_to_capture)?;
    rows.collect()
}

pub fn list_inputs(conn: &Connection, capture_id: &str) -> Result<Vec<CaptureInput>> {
    let mut stmt = conn.prepare(
        "SELECT id, capture_id, kind, filename, extracted_text, char_count FROM capture_inputs WHERE capture_id = ?1 ORDER BY rowid",
    )?;
    let rows = stmt.query_map([capture_id], |row| {
        let kind: String = row.get(2)?;
        Ok(CaptureInput {
            id: row.get(0)?,
            capture_id: row.get(1)?,
            kind: InputKind::from_str(&kind).map_err(parse_err)?,
            filename: row.get(3)?,
            extracted_text: row.get(4)?,
            char_count: row.get(5)?,
        })
    })?;
    rows.collect()
}

const CHANGE_COLS: &str = "id, capture_id, task_id, kind, before_json, after_json, reason, evidence, applied, undone, created_at";

fn row_to_change(row: &Row) -> rusqlite::Result<Change> {
    let kind: String = row.get(3)?;
    let before: String = row.get(4)?;
    let after: String = row.get(5)?;
    Ok(Change {
        id: row.get(0)?,
        capture_id: row.get(1)?,
        task_id: row.get(2)?,
        kind: ChangeKind::from_str(&kind).map_err(parse_err)?,
        before: serde_json::from_str(&before).map_err(|e| parse_err(e.to_string()))?,
        after: serde_json::from_str(&after).map_err(|e| parse_err(e.to_string()))?,
        reason: row.get(6)?,
        evidence: row.get(7)?,
        applied: row.get::<_, i64>(8)? != 0,
        undone: row.get::<_, i64>(9)? != 0,
        created_at: row.get(10)?,
    })
}

pub fn insert_change(conn: &Connection, c: &Change) -> Result<()> {
    conn.execute(
        &format!("INSERT INTO changes ({CHANGE_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"),
        params![
            c.id, c.capture_id, c.task_id, c.kind.as_str(), c.before.to_string(), c.after.to_string(),
            c.reason, c.evidence, c.applied as i64, c.undone as i64, c.created_at
        ],
    )?;
    Ok(())
}

pub fn get_change(conn: &Connection, id: &str) -> Result<Option<Change>> {
    conn.query_row(&format!("SELECT {CHANGE_COLS} FROM changes WHERE id = ?1"), [id], row_to_change)
        .optional()
}

pub fn list_changes_for_capture(conn: &Connection, capture_id: &str) -> Result<Vec<Change>> {
    let mut stmt = conn.prepare(&format!("SELECT {CHANGE_COLS} FROM changes WHERE capture_id = ?1 ORDER BY rowid"))?;
    let rows = stmt.query_map([capture_id], row_to_change)?;
    rows.collect()
}

pub fn list_changes_for_task(conn: &Connection, task_id: &str) -> Result<Vec<Change>> {
    let mut stmt = conn.prepare(&format!("SELECT {CHANGE_COLS} FROM changes WHERE task_id = ?1 ORDER BY rowid"))?;
    let rows = stmt.query_map([task_id], row_to_change)?;
    rows.collect()
}

pub fn mark_undone(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE changes SET undone = 1 WHERE id = ?1", [id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::ops::Proposal;
    use crate::db::open_memory;
    use crate::models::*;

    #[test]
    fn capture_lifecycle_roundtrips() {
        let conn = open_memory().unwrap();
        let cap = insert_capture(&conn, "claude-haiku-5-5").unwrap();
        assert_eq!(cap.status, CaptureStatus::Proposed);
        assert!(cap.proposal.is_none());
        insert_input(&conn, &cap.id, InputKind::Paste, None, "hello world").unwrap();
        insert_input(&conn, &cap.id, InputKind::File, Some("a.vtt"), "x".repeat(10).as_str()).unwrap();
        let inputs = list_inputs(&conn, &cap.id).unwrap();
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].kind, InputKind::Paste);
        assert_eq!(inputs[0].char_count, 11);
        assert_eq!(inputs[1].filename.as_deref(), Some("a.vtt"));

        let p = Proposal { summary: "sum".into(), ops: vec![], warnings: vec!["w".into()] };
        set_proposal(&conn, &cap.id, &p, (100, 20)).unwrap();
        let got = get_capture(&conn, &cap.id).unwrap().unwrap();
        assert_eq!(got.summary.as_deref(), Some("sum"));
        assert_eq!(got.input_tokens, 100);
        assert_eq!(got.output_tokens, 20);
        assert_eq!(got.proposal, Some(p));

        set_status(&conn, &cap.id, CaptureStatus::Failed).unwrap();
        assert_eq!(get_capture(&conn, &cap.id).unwrap().unwrap().status, CaptureStatus::Failed);
    }

    #[test]
    fn list_captures_newest_first_with_limit() {
        let conn = open_memory().unwrap();
        let a = insert_capture(&conn, "m").unwrap();
        conn.execute("UPDATE captures SET created_at = '2026-01-01T00:00:00Z' WHERE id = ?1", [&a.id]).unwrap();
        let b = insert_capture(&conn, "m").unwrap();
        let list = list_captures(&conn, 10).unwrap();
        assert_eq!(list.iter().map(|c| c.id.clone()).collect::<Vec<_>>(), vec![b.id.clone(), a.id.clone()]);
        assert_eq!(list_captures(&conn, 1).unwrap().len(), 1);
    }

    #[test]
    fn changes_roundtrip_and_mark_undone() {
        let conn = open_memory().unwrap();
        let cap = insert_capture(&conn, "m").unwrap();
        let c = Change {
            id: new_id(),
            capture_id: cap.id.clone(),
            task_id: Some("task-1".into()),
            kind: ChangeKind::Update,
            before: serde_json::json!({"priority": "medium"}),
            after: serde_json::json!({"priority": "high"}),
            reason: "r".into(),
            evidence: None,
            applied: true,
            undone: false,
            created_at: now_iso(),
        };
        insert_change(&conn, &c).unwrap();
        assert_eq!(get_change(&conn, &c.id).unwrap(), Some(c.clone()));
        assert_eq!(list_changes_for_capture(&conn, &cap.id).unwrap(), vec![c.clone()]);
        assert_eq!(list_changes_for_task(&conn, "task-1").unwrap().len(), 1);
        mark_undone(&conn, &c.id).unwrap();
        assert!(get_change(&conn, &c.id).unwrap().unwrap().undone);
    }
}
