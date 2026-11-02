use crate::ai::ops::ModelResponse;
use crate::ai::prompt::{build_body, ExtractionRequest};
use serde_json::{json, Value};
use std::time::Duration;

pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub const API_VERSION: &str = "2026-10-21";
pub const RETRY_BACKOFF_MS: [u64; 2] = [1000, 3000];
const TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Clone)]
pub struct AiConfig {
    pub api_key: String,
    pub model: String,
    pub effort: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AiUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("Add your Anthropic API key in Settings first.")]
    NoKey,
    #[error("Could not reach the Anthropic API: {0}")]
    Network(String),
    #[error("The API returned an error ({status}): {message}")]
    Api { status: u16, message: String },
    #[error("The model declined to process this context (category: {0}). Try editing the inputs.")]
    Refusal(String),
    #[error("The inputs were too large for one pass. Split them into two captures.")]
    Truncated,
    #[error("The model's answer could not be read: {0}")]
    BadJson(String),
}

/// POSTs to /v1/messages with retries on connection errors, 429 and 5xx.
pub(crate) async fn post_messages(cfg: &AiConfig, body: &Value, backoff_ms: &[u64]) -> Result<Value, AiError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .map_err(|e| AiError::Network(e.to_string()))?;
    let url = format!("{}/v1/messages", cfg.base_url.trim_end_matches('/'));
    let mut attempt = 0usize;
    loop {
        let result = client
            .post(&url)
            .header("x-api-key", &cfg.api_key)
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(body)
            .send()
            .await;
        let retryable_err = match result {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let text = resp.text().await.map_err(|e| AiError::Network(e.to_string()))?;
                if (200..300).contains(&status) {
                    return serde_json::from_str(&text).map_err(|e| AiError::BadJson(e.to_string()));
                }
                let message = serde_json::from_str::<Value>(&text)
                    .ok()
                    .and_then(|v| v["error"]["message"].as_str().map(|s| s.to_string()))
                    .unwrap_or_else(|| text.chars().take(200).collect());
                let err = AiError::Api { status, message };
                if status == 429 || status >= 500 { err } else { return Err(err) }
            }
            Err(e) => AiError::Network(e.to_string()),
        };
        if attempt >= backoff_ms.len() {
            return Err(retryable_err);
        }
        tokio::time::sleep(Duration::from_millis(backoff_ms[attempt])).await;
        attempt += 1;
    }
}

fn first_text(resp: &Value) -> Option<&str> {
    resp["content"]
        .as_array()?
        .iter()
        .find(|b| b["type"] == "text")
        .and_then(|b| b["text"].as_str())
}

fn usage_of(resp: &Value) -> AiUsage {
    AiUsage {
        input_tokens: resp["usage"]["input_tokens"].as_i64().unwrap_or(0),
        output_tokens: resp["usage"]["output_tokens"].as_i64().unwrap_or(0),
    }
}

fn check_stop(resp: &Value) -> Result<(), AiError> {
    match resp["stop_reason"].as_str() {
        Some("refusal") => Err(AiError::Refusal(
            resp["stop_details"]["category"].as_str().unwrap_or("unspecified").to_string(),
        )),
        Some("max_tokens") => Err(AiError::Truncated),
        _ => Ok(()),
    }
}

pub async fn run_extraction(cfg: &AiConfig, req: &ExtractionRequest) -> Result<(ModelResponse, AiUsage), AiError> {
    if cfg.api_key.trim().is_empty() {
        return Err(AiError::NoKey);
    }
    let body = build_body(&cfg.model, &cfg.effort, req);
    let resp = post_messages(cfg, &body, &RETRY_BACKOFF_MS).await?;
    check_stop(&resp)?;
    let text = first_text(&resp).ok_or_else(|| AiError::BadJson("no text block in response".to_string()))?;
    let parsed: ModelResponse = serde_json::from_str(text).map_err(|e| AiError::BadJson(e.to_string()))?;
    Ok((parsed, usage_of(&resp)))
}

/// Sends a tiny request; returns the model name the API reports.
pub async fn test_connection(cfg: &AiConfig) -> Result<String, AiError> {
    if cfg.api_key.trim().is_empty() {
        return Err(AiError::NoKey);
    }
    let body = json!({
        "model": cfg.model,
        "max_tokens": 64,
        "output_config": {"effort": "low"},
        "messages": [{"role": "user", "content": "Reply with OK."}]
    });
    let resp = post_messages(cfg, &body, &RETRY_BACKOFF_MS).await?;
    check_stop(&resp)?;
    Ok(resp["model"].as_str().unwrap_or(&cfg.model).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::prompt::ExtractionRequest;
    use httpmock::prelude::*;

    fn cfg(server: &MockServer) -> AiConfig {
        AiConfig {
            api_key: "sk-test".into(),
            model: "claude-haiku-5-5".into(),
            effort: "medium".into(),
            base_url: server.base_url(),
        }
    }

    fn req() -> ExtractionRequest {
        ExtractionRequest {
            today: "2026-11-01".into(),
            user_name: "Sam".into(),
            manager_name: String::new(),
            open_tasks: vec![],
            categories: vec![],
            inputs: vec![crate::ingest::Extracted::Text { filename: "a.txt".into(), text: "hi".into() }],
        }
    }

    const OPS_JSON: &str = r#"{"summary":"ok","operations":[]}"#;

    fn ok_body(text: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "msg_1", "type": "message", "role": "assistant", "model": "claude-haiku-5-5",
            "content": [
                {"type": "thinking", "thinking": "", "signature": "x"},
                {"type": "text", "text": text}
            ],
            "stop_reason": "end_turn", "stop_details": null,
            "usage": {"input_tokens": 120, "output_tokens": 30}
        })
    }

    #[tokio::test]
    async fn parses_text_block_after_thinking_block_and_sends_required_headers() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/messages")
                .header("x-api-key", "sk-test")
                .header("anthropic-version", "2026-10-21")
                .header("content-type", "application/json")
                .json_body_includes(r#"{"model": "claude-haiku-5-5", "max_tokens": 16000}"#);
            then.status(200).json_body(ok_body(OPS_JSON));
        });
        let (resp, usage) = run_extraction(&cfg(&server), &req()).await.unwrap();
        m.assert();
        assert_eq!(resp.summary, "ok");
        assert_eq!(usage.input_tokens, 120);
        assert_eq!(usage.output_tokens, 30);
    }

    #[tokio::test]
    async fn refusal_is_typed_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/messages");
            let mut b = ok_body("");
            b["stop_reason"] = serde_json::json!("refusal");
            b["stop_details"] = serde_json::json!({"type": "refusal", "category": "general_harms"});
            then.status(200).json_body(b);
        });
        let err = run_extraction(&cfg(&server), &req()).await.unwrap_err();
        assert!(matches!(err, AiError::Refusal(ref c) if c == "general_harms"), "{err:?}");
    }

    #[tokio::test]
    async fn max_tokens_is_truncated_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/messages");
            let mut b = ok_body("{\"summary\":\"cut");
            b["stop_reason"] = serde_json::json!("max_tokens");
            then.status(200).json_body(b);
        });
        let err = run_extraction(&cfg(&server), &req()).await.unwrap_err();
        assert!(matches!(err, AiError::Truncated), "{err:?}");
    }

    #[tokio::test]
    async fn bad_json_is_typed_error() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/messages");
            then.status(200).json_body(ok_body("not json"));
        });
        let err = run_extraction(&cfg(&server), &req()).await.unwrap_err();
        assert!(matches!(err, AiError::BadJson(_)), "{err:?}");
    }

    #[tokio::test]
    async fn api_error_message_surfaced_without_retry() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(POST).path("/v1/messages");
            then.status(400).json_body(serde_json::json!({"type": "error", "error": {"type": "invalid_request_error", "message": "bad"}}));
        });
        let err = run_extraction(&cfg(&server), &req()).await.unwrap_err();
        assert!(matches!(err, AiError::Api { status: 400, ref message } if message == "bad"), "{err:?}");
        assert_eq!(m.calls(), 1);
    }

    #[tokio::test]
    async fn connect_error_is_network_error() {
        let mut bad = AiConfig {
            api_key: "k".into(),
            model: "m".into(),
            effort: "low".into(),
            base_url: "http://127.0.0.1:1".into(),
        };
        bad.base_url = "http://127.0.0.1:1".into();
        let body = crate::ai::prompt::build_body(&bad.model, &bad.effort, &req());
        let err = post_messages(&bad, &body, &[0, 0]).await.unwrap_err();
        assert!(matches!(err, AiError::Network(_)), "{err:?}");
    }

    #[tokio::test]
    async fn retries_503_twice_then_gives_up() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(POST).path("/v1/messages");
            then.status(503).json_body(serde_json::json!({"error": {"message": "overloaded"}}));
        });
        let body = crate::ai::prompt::build_body("m", "low", &req());
        let err = post_messages(&cfg(&server), &body, &[0, 0]).await.unwrap_err();
        assert!(matches!(err, AiError::Api { status: 503, ref message } if message == "overloaded"), "{err:?}");
        assert_eq!(m.calls(), 3, "initial attempt + 2 retries");
    }

    #[test]
    fn default_backoff_is_1s_then_3s() {
        assert_eq!(RETRY_BACKOFF_MS, [1000, 3000]);
    }

    #[tokio::test]
    async fn test_connection_returns_model_name() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST).path("/v1/messages").json_body_includes(r#"{"max_tokens": 64}"#);
            then.status(200).json_body(ok_body("OK"));
        });
        let name = test_connection(&cfg(&server)).await.unwrap();
        assert_eq!(name, "claude-haiku-5-5");
    }
}
