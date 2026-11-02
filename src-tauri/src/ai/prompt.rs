use crate::ai::schema::operations_schema;
use crate::ingest::Extracted;
use crate::models::Task;
use serde_json::{json, Value};

pub const MAX_TOKENS: u32 = 16000;

/// Static so the prefix caches across captures; per-capture facts go in the user message.
pub const SYSTEM_PROMPT: &str = "You maintain one person's personal task list. The message gives you today's date, the user's name, optionally their manager's name, the list of open tasks with ids, the known categories, and one or more pieces of context (meeting transcripts, emails, chat messages, notes, or free text).\n\nFrom the context, produce the operations that keep the task list accurate:\n- Extract concrete tasks the user must do. Skip tasks that belong to other people unless the user has to follow up on them.\n- Prefer updating an existing task over creating a near-duplicate. Match by meaning, not exact wording. When the context adds information to an existing task, use add_note; when it changes the task's facts (priority, due date, title, description, parent), use update; when it says the task is finished, use complete.\n- Create sub-tasks (create with parent_id) only when the context clearly separates a task into steps. Keep one level: a sub-task never has its own sub-tasks.\n- Priority: urgent when it blocks others or is due within a day; high when the manager asked for it or it is due this week; medium by default; low for nice-to-haves. The manager's asks default to high.\n- Use an existing category when one fits; otherwise invent a short new one (one or two words).\n- Resolve relative dates (Friday, next week, end of month) against today's date and emit YYYY-MM-DD. Leave due_date null when no date was given or implied.\n- Titles are imperative and under 80 characters. Descriptions say what needs to happen and why, briefly.\n- Give a one-line reason per operation. Put a verbatim quote from the context (under 200 characters) in evidence when one supports the operation; otherwise null.\n- If the context contains nothing actionable, return an empty operations list and say so in the summary.\n- Never invent tasks that the context does not support.";

#[derive(Debug, Clone)]
pub struct ExtractionRequest {
    pub today: String,
    pub user_name: String,
    pub manager_name: String,
    pub open_tasks: Vec<Task>,
    pub categories: Vec<String>,
    pub inputs: Vec<Extracted>,
}

fn clip(s: &str, max: usize) -> String {
    let one_line: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= max {
        one_line
    } else {
        let mut t: String = one_line.chars().take(max).collect();
        t.push_str("...");
        t
    }
}

fn task_line(t: &Task) -> String {
    let mut line = format!(
        "- [{}] {} | {} | {} | {} | {}",
        t.id,
        t.title,
        t.status.as_str(),
        t.priority.as_str(),
        t.category.as_deref().unwrap_or("-"),
        t.due_date.as_deref().map(|d| format!("due {d}")).unwrap_or_else(|| "no due date".to_string()),
    );
    if let Some(p) = &t.parent_id {
        line.push_str(&format!(" | parent:{p}"));
    }
    if !t.description.trim().is_empty() {
        line.push_str("\n    ");
        line.push_str(&clip(&t.description, 120));
    }
    line
}

pub fn context_block(req: &ExtractionRequest) -> String {
    let mut s = String::new();
    s.push_str(&format!("Today: {}\n", req.today));
    s.push_str(&format!("User: {}\n", req.user_name));
    if !req.manager_name.trim().is_empty() {
        s.push_str(&format!("Manager: {}\n", req.manager_name));
    }
    s.push_str(&format!(
        "Categories: {}\n\n",
        if req.categories.is_empty() { "(none yet)".to_string() } else { req.categories.join(", ") }
    ));
    if req.open_tasks.is_empty() {
        s.push_str("Open tasks: none\n");
    } else {
        s.push_str("Open tasks (id | title | status | priority | category | due | parent):\n");
        for t in &req.open_tasks {
            s.push_str(&task_line(t));
            s.push('\n');
        }
    }
    s
}

pub fn build_user_content(req: &ExtractionRequest) -> Vec<Value> {
    let mut content = vec![json!({"type": "text", "text": context_block(req)})];
    for input in &req.inputs {
        match input {
            Extracted::Text { filename, text } => content.push(json!({
                "type": "text",
                "text": format!("<input name=\"{}\">\n{}\n</input>", filename, text)
            })),
            Extracted::Pdf { filename, base64, .. } => content.push(json!({
                "type": "document",
                "title": filename,
                "source": {"type": "base64", "media_type": "application/pdf", "data": base64}
            })),
        }
    }
    content.push(json!({"type": "text", "text": "Produce the operations JSON."}));
    content
}

/// The same request as one plain-text prompt, for providers that take stdin
/// instead of content blocks. PDFs are referred to by the staged path given.
pub fn user_text(req: &ExtractionRequest, pdf_paths: &[(String, String)]) -> String {
    let mut parts = vec![context_block(req).trim_end().to_string()];
    for input in &req.inputs {
        if let Extracted::Text { filename, text } = input {
            parts.push(format!("<input name=\"{filename}\">\n{text}\n</input>"));
        }
    }
    for (filename, path) in pdf_paths {
        parts.push(format!("Read the PDF file at {path} with the Read tool; it is the input named \"{filename}\"."));
    }
    parts.push("Produce the operations JSON.".to_string());
    parts.join("\n\n")
}

pub fn build_body(model: &str, effort: &str, req: &ExtractionRequest) -> Value {
    json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "system": [{"type": "text", "text": SYSTEM_PROMPT, "cache_control": {"type": "ephemeral"}}],
        "output_config": {"effort": effort, "format": {"type": "json_schema", "schema": operations_schema()}},
        "messages": [{"role": "user", "content": build_user_content(req)}]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::Extracted;
    use crate::models::{Priority, Status, Task};

    fn task() -> Task {
        Task {
            id: "abc-1".into(),
            title: "Send pricing".into(),
            description: "A long description ".repeat(20),
            status: Status::Todo,
            priority: Priority::High,
            category: Some("Pricing".into()),
            due_date: Some("2026-11-06".into()),
            parent_id: None,
            pinned: false,
            snoozed_until: None,
            created_at: "2026-11-01T00:00:00Z".into(),
            updated_at: "2026-11-01T00:00:00Z".into(),
            completed_at: None,
        }
    }

    fn req() -> ExtractionRequest {
        ExtractionRequest {
            today: "2026-11-01".into(),
            user_name: "Sam".into(),
            manager_name: "Alex Morgan".into(),
            open_tasks: vec![task()],
            categories: vec!["Pricing".into(), "Observability".into()],
            inputs: vec![
                Extracted::Text { filename: "notes.md".into(), text: "do the thing".into() },
                Extracted::Pdf { filename: "deck.pdf".into(), base64: "JVBERg==".into(), path: r"C:\tmp\deck.pdf".into() },
            ],
        }
    }

    #[test]
    fn body_has_required_shape_and_no_sampling_params() {
        let body = build_body("claude-haiku-5-5", "medium", &req());
        assert_eq!(body["model"], "claude-haiku-5-5");
        assert_eq!(body["max_tokens"], 16000);
        assert_eq!(body["output_config"]["effort"], "medium");
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert!(body["output_config"]["format"]["schema"]["properties"]["operations"].is_object());
        assert_eq!(body["system"][0]["type"], "text");
        assert_eq!(body["system"][0]["text"], SYSTEM_PROMPT);
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
        for absent in ["temperature", "top_p", "top_k", "thinking", "tools", "tool_choice"] {
            assert!(body.get(absent).is_none(), "{absent} must not be sent");
        }
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
    }

    #[test]
    fn context_block_lists_tasks_names_and_categories() {
        let body = build_body("m", "low", &req());
        let first = body["messages"][0]["content"][0]["text"].as_str().unwrap();
        assert!(first.contains("Today: 2026-11-01"));
        assert!(first.contains("User: Sam"));
        assert!(first.contains("Manager: Alex Morgan"));
        assert!(first.contains("- [abc-1] Send pricing | todo | high | Pricing | due 2026-11-06"));
        assert!(!first.contains(&"A long description ".repeat(20)), "description is clipped");
        assert!(first.contains("Categories: Pricing, Observability"));
    }

    #[test]
    fn pdf_input_becomes_document_block() {
        let body = build_body("m", "low", &req());
        let content = body["messages"][0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 4, "context + 2 inputs + final instruction");
        assert_eq!(content[1]["type"], "text");
        assert!(content[1]["text"].as_str().unwrap().starts_with("<input name=\"notes.md\">"));
        assert_eq!(content[2]["type"], "document");
        assert_eq!(content[2]["source"]["type"], "base64");
        assert_eq!(content[2]["source"]["media_type"], "application/pdf");
        assert_eq!(content[2]["source"]["data"], "JVBERg==");
        assert_eq!(content[3]["text"], "Produce the operations JSON.");
    }

    #[test]
    fn empty_manager_is_omitted() {
        let mut r = req();
        r.manager_name = String::new();
        let body = build_body("m", "low", &r);
        let first = body["messages"][0]["content"][0]["text"].as_str().unwrap();
        assert!(!first.contains("Manager:"));
    }

    #[test]
    fn user_text_lists_inputs_and_pdf_paths() {
        let r = req();
        let text = user_text(&r, &[("deck.pdf".to_string(), r"C:\tmp\deck.pdf".to_string())]);
        assert!(text.starts_with("Today: 2026-11-01"));
        assert!(text.contains("<input name=\"notes.md\">\ndo the thing\n</input>"));
        assert!(text.contains(r#"Read the PDF file at C:\tmp\deck.pdf with the Read tool; it is the input named "deck.pdf"."#));
        assert!(text.ends_with("Produce the operations JSON."));
        assert!(!text.contains("JVBERg=="), "the base64 never goes into the text");
    }

    #[test]
    fn api_body_unchanged_by_user_text() {
        let body = build_body("m", "low", &req());
        let content = body["messages"][0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 4);
        assert_eq!(content[2]["type"], "document");
        assert!(content[2].get("path").is_none(), "the API body never carries a local path");
    }
}
