use serde_json::{json, Value};

fn nullable_string(desc: &str) -> Value {
    json!({ "type": ["string", "null"], "description": desc })
}

/// JSON schema for the model's structured output: `{summary, operations[]}`.
/// Every object closes `additionalProperties` and lists every property as
/// required, as structured outputs demand; optional fields are nullable.
pub fn operations_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "summary": {
                "type": "string",
                "description": "One short paragraph: what this context was about and what changed."
            },
            "operations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "op": {
                            "type": "string",
                            "enum": ["create", "update", "complete", "add_note"],
                            "description": "create a new task; update fields of an existing task; complete an existing task; add_note attaches new information to an existing task."
                        },
                        "temp_id": nullable_string("For create only: a short id like t1, t2 so other creates can reference this task as their parent."),
                        "task_id": nullable_string("For update/complete/add_note: the id of the existing task, copied exactly from the open-task list."),
                        "title": nullable_string("Imperative, under 80 characters. Required for create; for update only when the title should change."),
                        "description": nullable_string("What needs to happen and why, in a few sentences. Markdown allowed."),
                        "priority": {
                            "anyOf": [
                                { "type": "string", "enum": ["urgent", "high", "medium", "low"] },
                                { "type": "null" }
                            ],
                            "description": "urgent = blocking or due within a day; high = manager asked or due this week; medium = normal; low = nice to have."
                        },
                        "category": nullable_string("Short label. Reuse an existing category when one fits."),
                        "due_date": nullable_string("YYYY-MM-DD, resolved against today's date, or null when no date was given."),
                        "parent_id": nullable_string("Existing task id or a temp_id from this response when this is a sub-task; otherwise null."),
                        "body": nullable_string("For add_note: the new information, one or two sentences."),
                        "reason": {
                            "type": "string",
                            "description": "One line: why this operation and, for create/update, why this priority."
                        },
                        "evidence": nullable_string("A verbatim quote from the context, under 200 characters, that supports this operation; null if none.")
                    },
                    "required": ["op", "temp_id", "task_id", "title", "description", "priority", "category", "due_date", "parent_id", "body", "reason", "evidence"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["summary", "operations"],
        "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use super::operations_schema;
    use serde_json::Value;

    fn walk(v: &Value, path: &str) {
        if let Some(obj) = v.as_object() {
            if obj.get("type").and_then(|t| t.as_str()) == Some("object") {
                assert_eq!(
                    obj.get("additionalProperties"),
                    Some(&Value::Bool(false)),
                    "{path} must close additionalProperties"
                );
                let mut props: Vec<String> = obj["properties"].as_object().unwrap().keys().cloned().collect();
                let mut req: Vec<String> = obj["required"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap().to_string())
                    .collect();
                props.sort();
                req.sort();
                assert_eq!(props, req, "{path}: required must list every property");
            }
            for (k, child) in obj {
                walk(child, &format!("{path}/{k}"));
            }
        } else if let Some(arr) = v.as_array() {
            for (i, child) in arr.iter().enumerate() {
                walk(child, &format!("{path}[{i}]"));
            }
        }
    }

    #[test]
    fn schema_has_no_open_objects() {
        let s = operations_schema();
        walk(&s, "root");
        let op = &s["properties"]["operations"]["items"]["properties"];
        for key in [
            "op", "temp_id", "task_id", "title", "description", "priority", "category", "due_date", "parent_id",
            "body", "reason", "evidence",
        ] {
            assert!(op.get(key).is_some(), "missing {key}");
        }
        assert_eq!(op["op"]["enum"], serde_json::json!(["create", "update", "complete", "add_note"]));
        assert_eq!(op["priority"]["anyOf"][0]["enum"], serde_json::json!(["urgent", "high", "medium", "low"]));
    }
}
