use crate::models::{NewTask, Priority, TaskPatch};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

const MAX_TITLE_CHARS: usize = 120;

/// One operation exactly as the model emits it (every field present, nullable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawOp {
    pub op: String,
    pub temp_id: Option<String>,
    pub task_id: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub category: Option<String>,
    pub due_date: Option<String>,
    pub parent_id: Option<String>,
    pub body: Option<String>,
    pub reason: String,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelResponse {
    pub summary: String,
    pub operations: Vec<RawOp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResolvedOp {
    Create {
        temp_id: Option<String>,
        new: NewTask,
        /// Set when the parent is another create in the same proposal.
        parent_temp: Option<String>,
    },
    Update {
        task_id: String,
        patch: TaskPatch,
    },
    Complete {
        task_id: String,
    },
    AddNote {
        task_id: String,
        body: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedOp {
    pub index: usize,
    pub op: ResolvedOp,
    pub reason: String,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    pub summary: String,
    pub ops: Vec<ProposedOp>,
    pub warnings: Vec<String>,
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn valid_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
}

fn clip_title(title: &str) -> String {
    if title.chars().count() <= MAX_TITLE_CHARS {
        return title.to_string();
    }
    let mut t: String = title.chars().take(MAX_TITLE_CHARS - 1).collect();
    t.push('\u{2026}');
    t
}

/// Picks the parent for a task the model wants under `p`. Tasks nest one
/// level, so a parent that is itself a sub-task is replaced by its own parent
/// (or top level when that parent is not an open task).
fn pick_parent(
    p: &str,
    existing: &HashMap<String, Option<String>>,
    label: &str,
    warnings: &mut Vec<String>,
) -> Option<String> {
    match existing.get(p)? {
        None => Some(p.to_string()),
        Some(grand) => {
            let target = existing.get(grand).map(|_| grand.clone());
            warnings.push(format!(
                "{label}: {p:?} is already a sub-task and tasks nest one level, so {}",
                if target.is_some() { "used its parent instead" } else { "created at top level" }
            ));
            target
        }
    }
}

/// Validates the model's operations against the existing open tasks (id to
/// parent id) and turns them into typed operations. Invalid operations are
/// dropped with a warning; invalid fields are dropped or defaulted with a warning.
pub fn resolve(resp: ModelResponse, existing: &HashMap<String, Option<String>>) -> Proposal {
    let mut warnings = Vec::new();
    let temp_ids: HashSet<String> = resp
        .operations
        .iter()
        .filter(|o| o.op == "create")
        .filter_map(|o| non_empty(o.temp_id.clone()))
        .collect();
    let mut ops = Vec::new();

    for (index, raw) in resp.operations.into_iter().enumerate() {
        let label = raw
            .title
            .clone()
            .or(raw.task_id.clone())
            .unwrap_or_else(|| format!("operation {}", index + 1));
        let reason = raw.reason.trim().to_string();
        let evidence = non_empty(raw.evidence.clone());

        let priority = match raw.priority.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(p) => match Priority::from_str(&p.to_ascii_lowercase()) {
                Ok(v) => Some(v),
                Err(_) => {
                    warnings.push(format!("{label}: unknown priority {p:?}, used medium"));
                    Some(Priority::Medium)
                }
            },
        };
        let due_date = match non_empty(raw.due_date.clone()) {
            Some(d) if valid_date(&d) => Some(d),
            Some(d) => {
                warnings.push(format!("{label}: ignored due date {d:?} (not YYYY-MM-DD)"));
                None
            }
            None => None,
        };

        let op = match raw.op.as_str() {
            "create" => {
                let Some(title) = non_empty(raw.title.clone()) else {
                    warnings.push(format!("{label}: create without a title was skipped"));
                    continue;
                };
                let mut parent_id = None;
                let mut parent_temp = None;
                if let Some(p) = non_empty(raw.parent_id.clone()) {
                    if existing.contains_key(&p) {
                        parent_id = pick_parent(&p, existing, &title, &mut warnings);
                    } else if temp_ids.contains(&p) && raw.temp_id.as_deref() != Some(p.as_str()) {
                        parent_temp = Some(p);
                    } else {
                        warnings.push(format!("{title}: parent {p:?} not found, created at top level"));
                    }
                }
                ResolvedOp::Create {
                    temp_id: non_empty(raw.temp_id.clone()),
                    new: NewTask {
                        title: clip_title(&title),
                        description: non_empty(raw.description.clone()).unwrap_or_default(),
                        priority: priority.unwrap_or_default(),
                        category: non_empty(raw.category.clone()),
                        due_date,
                        parent_id,
                    },
                    parent_temp,
                }
            }
            "update" | "complete" | "add_note" => {
                let Some(task_id) = non_empty(raw.task_id.clone()).filter(|id| existing.contains_key(id)) else {
                    warnings.push(format!("{label}: {} refers to a task that does not exist, skipped", raw.op));
                    continue;
                };
                match raw.op.as_str() {
                    "complete" => ResolvedOp::Complete { task_id },
                    "add_note" => {
                        let Some(body) = non_empty(raw.body.clone()) else {
                            warnings.push(format!("{label}: empty note was skipped"));
                            continue;
                        };
                        ResolvedOp::AddNote { task_id, body }
                    }
                    _ => {
                        let mut parent_patch: Option<Option<String>> = None;
                        if let Some(p) = non_empty(raw.parent_id.clone()) {
                            if existing.contains_key(&p) && p != task_id {
                                parent_patch = pick_parent(&p, existing, &label, &mut warnings).map(Some);
                            } else {
                                warnings.push(format!("{label}: parent {p:?} not found, parent unchanged"));
                            }
                        }
                        let patch = TaskPatch {
                            title: non_empty(raw.title.clone()).map(|t| clip_title(&t)),
                            description: non_empty(raw.description.clone()),
                            status: None,
                            priority,
                            category: non_empty(raw.category.clone()).map(Some),
                            due_date: due_date.map(Some),
                            parent_id: parent_patch,
                            pinned: None,
                            snoozed_until: None,
                        };
                        if patch == TaskPatch::default() {
                            continue;
                        }
                        ResolvedOp::Update { task_id, patch }
                    }
                }
            }
            other => {
                warnings.push(format!("{label}: unknown operation {other:?} was skipped"));
                continue;
            }
        };
        ops.push(ProposedOp { index, op, reason, evidence });
    }

    Proposal { summary: resp.summary.trim().to_string(), ops, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Priority;

    fn ids(v: &[&str]) -> HashMap<String, Option<String>> {
        v.iter().map(|s| (s.to_string(), None)).collect()
    }

    fn raw(op: &str) -> RawOp {
        RawOp {
            op: op.to_string(),
            temp_id: None,
            task_id: None,
            title: None,
            description: None,
            priority: None,
            category: None,
            due_date: None,
            parent_id: None,
            body: None,
            reason: "because".to_string(),
            evidence: None,
        }
    }

    #[test]
    fn create_with_temp_parent_links() {
        let mut parent = raw("create");
        parent.temp_id = Some("t1".into());
        parent.title = Some("Big thing".into());
        parent.priority = Some("high".into());
        let mut child = raw("create");
        child.temp_id = Some("t2".into());
        child.title = Some("Step one".into());
        child.parent_id = Some("t1".into());
        let mut child_of_existing = raw("create");
        child_of_existing.title = Some("Step two".into());
        child_of_existing.parent_id = Some("existing-1".into());
        let resp = ModelResponse { summary: "s".into(), operations: vec![parent, child, child_of_existing] };
        let p = resolve(resp, &ids(&["existing-1"]));
        assert!(p.warnings.is_empty(), "{:?}", p.warnings);
        assert_eq!(p.ops.len(), 3);
        match &p.ops[0].op {
            ResolvedOp::Create { temp_id, new, parent_temp } => {
                assert_eq!(temp_id.as_deref(), Some("t1"));
                assert_eq!(new.priority, Priority::High);
                assert!(parent_temp.is_none());
            }
            _ => panic!(),
        }
        match &p.ops[1].op {
            ResolvedOp::Create { new, parent_temp, .. } => {
                assert_eq!(parent_temp.as_deref(), Some("t1"));
                assert!(new.parent_id.is_none());
            }
            _ => panic!(),
        }
        match &p.ops[2].op {
            ResolvedOp::Create { new, parent_temp, .. } => {
                assert_eq!(new.parent_id.as_deref(), Some("existing-1"));
                assert!(parent_temp.is_none());
            }
            _ => panic!(),
        }
        assert_eq!(p.ops[2].index, 2);
    }

    #[test]
    fn unknown_parent_is_warned_and_dropped() {
        let mut c = raw("create");
        c.title = Some("x".into());
        c.parent_id = Some("ghost".into());
        let p = resolve(ModelResponse { summary: "".into(), operations: vec![c] }, &ids(&[]));
        assert_eq!(p.ops.len(), 1);
        assert_eq!(p.warnings.len(), 1);
        match &p.ops[0].op {
            ResolvedOp::Create { new, parent_temp, .. } => {
                assert!(new.parent_id.is_none() && parent_temp.is_none());
            }
            _ => panic!(),
        }
    }

    #[test]
    fn update_unknown_task_is_warned_and_skipped() {
        let mut u = raw("update");
        u.task_id = Some("nope".into());
        u.priority = Some("urgent".into());
        let mut c = raw("complete");
        c.task_id = Some("nope".into());
        let mut n = raw("add_note");
        n.task_id = Some("nope".into());
        n.body = Some("hi".into());
        let p = resolve(ModelResponse { summary: "".into(), operations: vec![u, c, n] }, &ids(&["real"]));
        assert!(p.ops.is_empty());
        assert_eq!(p.warnings.len(), 3);
    }

    #[test]
    fn update_known_task_builds_patch() {
        let mut u = raw("update");
        u.task_id = Some("real".into());
        u.priority = Some("urgent".into());
        u.due_date = Some("2026-11-05".into());
        let p = resolve(ModelResponse { summary: "".into(), operations: vec![u] }, &ids(&["real"]));
        match &p.ops[0].op {
            ResolvedOp::Update { task_id, patch } => {
                assert_eq!(task_id, "real");
                assert_eq!(patch.priority, Some(Priority::Urgent));
                assert_eq!(patch.due_date, Some(Some("2026-11-05".into())));
                assert_eq!(patch.title, None);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn bad_priority_defaults_medium_with_warning() {
        let mut c = raw("create");
        c.title = Some("x".into());
        c.priority = Some("critical".into());
        let p = resolve(ModelResponse { summary: "".into(), operations: vec![c] }, &ids(&[]));
        match &p.ops[0].op {
            ResolvedOp::Create { new, .. } => assert_eq!(new.priority, Priority::Medium),
            _ => panic!(),
        }
        assert_eq!(p.warnings.len(), 1);
    }

    #[test]
    fn bad_date_dropped() {
        let mut c = raw("create");
        c.title = Some("x".into());
        c.due_date = Some("next Friday".into());
        let p = resolve(ModelResponse { summary: "".into(), operations: vec![c] }, &ids(&[]));
        match &p.ops[0].op {
            ResolvedOp::Create { new, .. } => assert!(new.due_date.is_none()),
            _ => panic!(),
        }
        assert_eq!(p.warnings.len(), 1);
    }

    #[test]
    fn empty_update_and_untitled_create_skipped() {
        let mut u = raw("update");
        u.task_id = Some("real".into());
        let c = raw("create");
        let p = resolve(ModelResponse { summary: "".into(), operations: vec![u, c] }, &ids(&["real"]));
        assert!(p.ops.is_empty());
        assert_eq!(p.warnings.len(), 1, "untitled create warns; empty update is silent");
    }

    #[test]
    fn long_title_truncated() {
        let mut c = raw("create");
        c.title = Some("x".repeat(200));
        let p = resolve(ModelResponse { summary: "".into(), operations: vec![c] }, &ids(&[]));
        match &p.ops[0].op {
            ResolvedOp::Create { new, .. } => {
                assert_eq!(new.title.chars().count(), 120);
                assert!(new.title.ends_with('\u{2026}'));
            }
            _ => panic!(),
        }
    }

    #[test]
    fn raw_json_from_model_deserializes() {
        let json = r#"{"summary":"Meeting about pricing","operations":[
          {"op":"create","temp_id":"t1","task_id":null,"title":"Send pricing","description":"to Alex","priority":"high","category":"Pricing","due_date":"2026-11-06","parent_id":null,"body":null,"reason":"asked by manager","evidence":"send the pricing by Friday"},
          {"op":"complete","temp_id":null,"task_id":"abc","title":null,"description":null,"priority":null,"category":null,"due_date":null,"parent_id":null,"body":null,"reason":"done in call","evidence":null}
        ]}"#;
        let resp: ModelResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.operations.len(), 2);
        let p = resolve(resp, &ids(&["abc"]));
        assert_eq!(p.summary, "Meeting about pricing");
        assert_eq!(p.ops.len(), 2);
        assert_eq!(p.ops[0].evidence.as_deref(), Some("send the pricing by Friday"));
        assert!(matches!(p.ops[1].op, ResolvedOp::Complete { .. }));
        let round: Proposal = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(round, p);
    }


    fn ids_with_parents(v: &[(&str, Option<&str>)]) -> HashMap<String, Option<String>> {
        v.iter().map(|(id, p)| (id.to_string(), p.map(|s| s.to_string()))).collect()
    }

    #[test]
    fn create_under_a_subtask_is_reparented_to_its_parent() {
        let mut c = raw("create");
        c.title = Some("Grandchild".into());
        c.parent_id = Some("sub".into());
        let resp = ModelResponse { summary: "".into(), operations: vec![c] };
        let p = resolve(resp, &ids_with_parents(&[("top", None), ("sub", Some("top"))]));
        assert_eq!(p.ops.len(), 1);
        match &p.ops[0].op {
            ResolvedOp::Create { new, .. } => assert_eq!(new.parent_id.as_deref(), Some("top")),
            _ => panic!(),
        }
        assert_eq!(p.warnings.len(), 1, "{:?}", p.warnings);
        assert!(p.warnings[0].contains("one level"), "{:?}", p.warnings);
    }

    #[test]
    fn update_parent_to_a_subtask_is_reparented_to_its_parent() {
        let mut u = raw("update");
        u.task_id = Some("x".into());
        u.parent_id = Some("sub".into());
        let resp = ModelResponse { summary: "".into(), operations: vec![u] };
        let p = resolve(resp, &ids_with_parents(&[("top", None), ("sub", Some("top")), ("x", None)]));
        match &p.ops[0].op {
            ResolvedOp::Update { patch, .. } => assert_eq!(patch.parent_id, Some(Some("top".into()))),
            _ => panic!(),
        }
        assert_eq!(p.warnings.len(), 1, "{:?}", p.warnings);
    }

    #[test]
    fn create_under_a_subtask_whose_parent_is_closed_goes_top_level() {
        let mut c = raw("create");
        c.title = Some("Grandchild".into());
        c.parent_id = Some("sub".into());
        let resp = ModelResponse { summary: "".into(), operations: vec![c] };
        let p = resolve(resp, &ids_with_parents(&[("sub", Some("done-parent"))]));
        match &p.ops[0].op {
            ResolvedOp::Create { new, .. } => assert!(new.parent_id.is_none()),
            _ => panic!(),
        }
    }
}
