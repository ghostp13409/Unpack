use crate::models::{Priority, Status, Task};
use chrono::NaiveDate;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ranked {
    pub task_id: String,
    pub score: i32,
    pub reasons: Vec<String>,
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn is_open(t: &Task) -> bool {
    matches!(t.status, Status::Todo | Status::InProgress)
}

fn has_open_child(task: &Task, all: &[Task]) -> bool {
    all.iter().any(|c| c.parent_id.as_deref() == Some(task.id.as_str()) && is_open(c))
}

fn all_children_done(task: &Task, all: &[Task]) -> bool {
    let mut any = false;
    for c in all.iter().filter(|c| c.parent_id.as_deref() == Some(task.id.as_str())) {
        if c.status == Status::Dropped {
            continue;
        }
        any = true;
        if c.status != Status::Done {
            return false;
        }
    }
    any
}

pub fn score(task: &Task, all: &[Task], today: NaiveDate) -> (i32, Vec<String>) {
    let mut s = 0;
    let mut reasons = Vec::new();

    if task.pinned {
        s += 50;
        reasons.push("Pinned".to_string());
    }

    let (p, label) = match task.priority {
        Priority::Urgent => (40, "Urgent"),
        Priority::High => (25, "High"),
        Priority::Medium => (10, "Medium"),
        Priority::Low => (0, "Low"),
    };
    s += p;
    reasons.push(label.to_string());

    if let Some(due) = task.due_date.as_deref().and_then(parse_date) {
        let days = (due - today).num_days();
        if days < 0 {
            let overdue = -days;
            s += 30 + (overdue * 2).min(20) as i32;
            reasons.push(format!("Overdue by {overdue} day{}", if overdue == 1 { "" } else { "s" }));
        } else if days == 0 {
            s += 25;
            reasons.push("Due today".to_string());
        } else if days <= 3 {
            s += 15;
            reasons.push(format!("Due in {days} day{}", if days == 1 { "" } else { "s" }));
        } else if days <= 7 {
            s += 8;
            reasons.push(format!("Due in {days} days"));
        }
    }

    if task.status == Status::InProgress {
        s += 10;
        reasons.push("In progress".to_string());
    }

    if let Some(created) = task.created_at.get(0..10).and_then(parse_date) {
        let age = (today - created).num_days().max(0);
        let bonus = (age / 3).min(10) as i32;
        if bonus > 0 {
            s += bonus;
            reasons.push(format!("Open {age} days"));
        }
    }

    if all_children_done(task, all) {
        s += 15;
        reasons.push("All sub-tasks done".to_string());
    }

    (s, reasons)
}

pub fn up_next(all: &[Task], today: NaiveDate, limit: usize) -> Vec<Ranked> {
    let mut ranked: Vec<(Ranked, &str)> = all
        .iter()
        .filter(|t| is_open(t))
        .filter(|t| match t.snoozed_until.as_deref().and_then(parse_date) {
            Some(d) => d <= today,
            None => true,
        })
        .filter(|t| !has_open_child(t, all))
        .map(|t| {
            let (score, reasons) = score(t, all, today);
            (Ranked { task_id: t.id.clone(), score, reasons }, t.created_at.as_str())
        })
        .collect();
    ranked.sort_by(|a, b| b.0.score.cmp(&a.0.score).then(a.1.cmp(b.1)).then(a.0.task_id.cmp(&b.0.task_id)));
    ranked.into_iter().map(|(r, _)| r).take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use chrono::NaiveDate;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()
    }

    fn task(id: &str, priority: Priority) -> Task {
        Task {
            id: id.to_string(),
            title: id.to_string(),
            description: String::new(),
            status: Status::Todo,
            priority,
            category: None,
            due_date: None,
            parent_id: None,
            pinned: false,
            snoozed_until: None,
            created_at: "2026-11-01T00:00:00Z".to_string(),
            updated_at: "2026-11-01T00:00:00Z".to_string(),
            completed_at: None,
        }
    }

    #[test]
    fn overdue_high_outranks_fresh_urgent() {
        let urgent = task("u", Priority::Urgent);
        let mut high = task("h", Priority::High);
        high.due_date = Some("2026-10-30".into());
        let all = vec![urgent.clone(), high.clone()];
        let (su, ru) = score(&urgent, &all, today());
        let (sh, rh) = score(&high, &all, today());
        assert_eq!(su, 40);
        assert_eq!(ru, vec!["Urgent".to_string()]);
        assert_eq!(sh, 25 + 30 + 4);
        assert_eq!(rh, vec!["High".to_string(), "Overdue by 2 days".to_string()]);
        let ranked = up_next(&all, today(), 5);
        assert_eq!(ranked[0].task_id, "h");
    }

    #[test]
    fn overdue_bonus_caps_at_20_extra() {
        let mut t = task("t", Priority::Low);
        t.due_date = Some("2026-09-01".into());
        let (s, _) = score(&t, &[t.clone()], today());
        assert_eq!(s, 30 + 20);
    }

    #[test]
    fn due_today_and_soon_buckets() {
        let mut a = task("a", Priority::Low);
        a.due_date = Some("2026-11-01".into());
        let mut b = task("b", Priority::Low);
        b.due_date = Some("2026-11-03".into());
        let mut c = task("c", Priority::Low);
        c.due_date = Some("2026-11-07".into());
        let mut d = task("d", Priority::Low);
        d.due_date = Some("2026-11-20".into());
        let all = vec![a.clone(), b.clone(), c.clone(), d.clone()];
        assert_eq!(score(&a, &all, today()), (25, vec!["Low".to_string(), "Due today".to_string()]));
        assert_eq!(score(&b, &all, today()), (15, vec!["Low".to_string(), "Due in 2 days".to_string()]));
        assert_eq!(score(&c, &all, today()), (8, vec!["Low".to_string(), "Due in 6 days".to_string()]));
        assert_eq!(score(&d, &all, today()), (0, vec!["Low".to_string()]));
    }

    #[test]
    fn pinned_wins() {
        let urgent = task("u", Priority::Urgent);
        let mut low = task("l", Priority::Low);
        low.pinned = true;
        let all = vec![urgent, low.clone()];
        let (s, r) = score(&low, &all, today());
        assert_eq!(s, 50);
        assert_eq!(r[0], "Pinned");
        assert_eq!(up_next(&all, today(), 5)[0].task_id, "l");
    }

    #[test]
    fn snoozed_excluded() {
        let mut t = task("t", Priority::Urgent);
        t.snoozed_until = Some("2026-11-03".into());
        let mut past = task("p", Priority::Low);
        past.snoozed_until = Some("2026-11-01".into());
        let all = vec![t, past];
        let ids: Vec<_> = up_next(&all, today(), 5).into_iter().map(|r| r.task_id).collect();
        assert_eq!(ids, vec!["p".to_string()]);
    }

    #[test]
    fn parent_with_open_child_excluded_child_included() {
        let parent = task("parent", Priority::Urgent);
        let mut child = task("child", Priority::Low);
        child.parent_id = Some("parent".into());
        let all = vec![parent, child];
        let ids: Vec<_> = up_next(&all, today(), 5).into_iter().map(|r| r.task_id).collect();
        assert_eq!(ids, vec!["child".to_string()]);
    }

    #[test]
    fn parent_with_all_children_done_gets_bonus_15() {
        let parent = task("parent", Priority::Low);
        let mut child = task("child", Priority::Low);
        child.parent_id = Some("parent".into());
        child.status = Status::Done;
        let all = vec![parent.clone(), child];
        let (s, r) = score(&parent, &all, today());
        assert_eq!(s, 15);
        assert!(r.contains(&"All sub-tasks done".to_string()));
        assert_eq!(up_next(&all, today(), 5).len(), 1);
    }

    #[test]
    fn in_progress_and_age() {
        let mut t = task("t", Priority::Low);
        t.status = Status::InProgress;
        t.created_at = "2026-10-26T00:00:00Z".into(); // 6 days -> +2
        let (s, r) = score(&t, &[t.clone()], today());
        assert_eq!(s, 10 + 2);
        assert_eq!(r, vec!["Low".to_string(), "In progress".to_string(), "Open 6 days".to_string()]);
    }

    #[test]
    fn age_caps_at_10() {
        let mut t = task("t", Priority::Low);
        t.created_at = "2026-01-01T00:00:00Z".into();
        let (s, _) = score(&t, &[t.clone()], today());
        assert_eq!(s, 10);
    }

    #[test]
    fn limit_respected_and_done_excluded() {
        let mut all: Vec<Task> = (0..7).map(|i| task(&format!("t{i}"), Priority::Medium)).collect();
        all[0].status = Status::Done;
        let r = up_next(&all, today(), 3);
        assert_eq!(r.len(), 3);
        assert!(r.iter().all(|x| x.task_id != "t0"));
        assert_eq!(r[0].task_id, "t1", "ties break by created_at then id");
    }
}
