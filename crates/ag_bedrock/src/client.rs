//! Amazon Bedrock Runtime `Converse` HTTP client (API key / bearer auth).

use std::fmt;
use std::time::Duration;

use ag_credentials::{BedrockCredentials, DEFAULT_BEDROCK_REGION};
use reqwest::Client as ReqwestClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::context::{format_pack_for_prompt, ContextPack};
use crate::error::BedrockError;

/// Default Claude model on Bedrock (override later if needed).
pub const DEFAULT_MODEL: &str = "anthropic.claude-3-5-sonnet-20241022-v2:0";

/// System instruction: ground answers in the local context pack only.
pub const SYSTEM_INSTRUCTION: &str = "You are an analytics assistant for a local Jira engineering intelligence app. \
Answer ONLY using the provided context pack. Cite issue keys (e.g. PROJ-1) and metric names \
(e.g. bottleneck:Code Review, cycle_p50_secs) that support your answer. \
If the context pack does not contain enough information, say you don't know and do not invent data. \
Respond as JSON with keys \"text\" (string) and \"citations\" (array of strings).";

/// Grounded answer from Bedrock with citation strings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiAnswer {
    pub text: String,
    pub citations: Vec<String>,
}

/// Bedrock Converse client (base URL injectable for tests).
pub struct BedrockClient {
    api_key: String,
    base_url: String,
    model: String,
    http: ReqwestClient,
}

impl fmt::Debug for BedrockClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BedrockClient")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl BedrockClient {
    /// Production client talking to Bedrock Runtime with bearer API key auth.
    pub fn new(creds: &BedrockCredentials) -> Result<Self, BedrockError> {
        let creds = creds.clone().normalized();
        if creds.api_key.trim().is_empty() {
            return Err(BedrockError::MissingApiKey);
        }
        let region = if creds.region.trim().is_empty() {
            DEFAULT_BEDROCK_REGION
        } else {
            creds.region.trim()
        };
        let base_url = format!("https://bedrock-runtime.{region}.amazonaws.com");
        Self::with_base_url(creds.api_key.clone(), base_url, DEFAULT_MODEL.into())
    }

    /// Test helper with mock server base URL.
    pub fn new_for_test(base_url: &str, api_key: &str) -> Result<Self, BedrockError> {
        Self::with_base_url(
            api_key.into(),
            base_url.trim_end_matches('/').into(),
            DEFAULT_MODEL.into(),
        )
    }

    fn with_base_url(
        api_key: String,
        base_url: String,
        model: String,
    ) -> Result<Self, BedrockError> {
        let http = ReqwestClient::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| BedrockError::Http(e.to_string()))?;
        Ok(Self {
            api_key,
            base_url,
            model,
            http,
        })
    }

    fn converse_url(&self) -> String {
        format!("{}/model/{}/converse", self.base_url, self.model)
    }

    /// Ask a question grounded in `pack`. Failures are isolated to AI — callers must not affect dashboards.
    pub async fn ask(&self, pack: &ContextPack, question: &str) -> Result<AiAnswer, BedrockError> {
        let q = question.trim();
        if q.is_empty() {
            return Err(BedrockError::Other("question must not be empty".into()));
        }

        let context = format_pack_for_prompt(pack);
        let user_text =
            format!("Context pack:\n{context}\n\nQuestion: {q}\n\nRespond with JSON only.");

        let body = json!({
            "system": [{ "text": SYSTEM_INSTRUCTION }],
            "messages": [{
                "role": "user",
                "content": [{ "text": user_text }]
            }],
            "inferenceConfig": {
                "temperature": 0.2,
                "maxTokens": 4096
            }
        });

        let resp = self
            .http
            .post(self.converse_url())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| self.http_err(e))?;

        let status = resp.status().as_u16();
        let bytes = resp.bytes().await.map_err(|e| self.http_err(e))?;

        if !(200..300).contains(&status) {
            return Err(BedrockError::Api { status });
        }

        let envelope: Value =
            serde_json::from_slice(&bytes).map_err(|e| BedrockError::Parse(e.to_string()))?;
        let text = extract_assistant_text(&envelope)?;
        parse_answer(&text)
    }

    /// Lightweight connectivity probe (tiny Converse call).
    pub async fn probe(&self) -> Result<String, BedrockError> {
        let body = json!({
            "messages": [{
                "role": "user",
                "content": [{ "text": "Reply with the single word ok." }]
            }],
            "inferenceConfig": {
                "temperature": 0.0,
                "maxTokens": 8
            }
        });

        let resp = self
            .http
            .post(self.converse_url())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| self.http_err(e))?;
        let status = resp.status().as_u16();
        if (200..300).contains(&status) {
            Ok("bedrock converse reachable".into())
        } else {
            Err(BedrockError::Api { status })
        }
    }

    /// Map reqwest errors without embedding the API key.
    fn http_err(&self, err: reqwest::Error) -> BedrockError {
        BedrockError::Http(redact_secret(&err.to_string(), &self.api_key))
    }
}

/// Strip `secret` from error/display strings so it never reaches UI surfaces.
fn redact_secret(message: &str, secret: &str) -> String {
    if secret.is_empty() {
        return message.to_string();
    }
    message.replace(secret, "[REDACTED]")
}

fn extract_assistant_text(envelope: &Value) -> Result<String, BedrockError> {
    let content = envelope
        .pointer("/output/message/content")
        .and_then(|v| v.as_array())
        .ok_or_else(|| BedrockError::Parse("missing output.message.content".into()))?;

    let mut parts = Vec::new();
    for block in content {
        if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
            parts.push(text.to_string());
        }
    }
    if parts.is_empty() {
        return Err(BedrockError::Parse(
            "missing text in output.message.content".into(),
        ));
    }
    Ok(parts.join("\n"))
}

fn parse_answer(raw: &str) -> Result<AiAnswer, BedrockError> {
    let trimmed = raw.trim();
    // Strip optional markdown fences.
    let json_str = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .map(|s| s.strip_suffix("```").unwrap_or(s).trim())
        .unwrap_or(trimmed);

    if let Ok(value) = serde_json::from_str::<Value>(json_str) {
        let text = value
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or(trimmed)
            .to_string();
        let citations = value
            .get("citations")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| c.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        return Ok(AiAnswer { text, citations });
    }

    // Fallback: plain text with heuristic citations (ISSUE-123 / bottleneck:Name).
    let citations = extract_citations_heuristic(trimmed);
    Ok(AiAnswer {
        text: trimmed.to_string(),
        citations,
    })
}

fn extract_citations_heuristic(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in text.split_whitespace() {
        let cleaned = token
            .trim_matches(|c: char| matches!(c, ',' | '.' | ';' | ':' | ')' | '(' | '"' | '\''));
        if cleaned.contains('-')
            && cleaned
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
            && cleaned.chars().any(|c| c.is_ascii_digit())
            && cleaned.chars().any(|c| c.is_ascii_uppercase())
            && !out.contains(&cleaned.to_string())
        {
            out.push(cleaned.to_string());
        }
        if let Some(rest) = cleaned.strip_prefix("bottleneck:") {
            if !rest.is_empty() {
                let cite = format!("bottleneck:{rest}");
                if !out.contains(&cite) {
                    out.push(cite);
                }
            }
        }
    }
    out
}

/// Suggested prompts shown in Ask AI UI.
pub fn suggested_prompts() -> Vec<String> {
    vec![
        "What is our biggest flow bottleneck right now?".into(),
        "Which epics are most at risk and why?".into(),
        "How has throughput/velocity changed recently?".into(),
        "Where is capacity being consumed (reopens, handoffs, waiting)?".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::IssueCite;

    fn sample_pack() -> ContextPack {
        ContextPack {
            filter_summary: "projects=PROJ; dates=all; types=all; assignees=all".into(),
            metrics_markdown: "- bottleneck:Code Review = 90000s\n- cycle_p50_secs: 86400\n".into(),
            supporting_issues: vec![IssueCite {
                key: "PROJ-1".into(),
                summary: Some("Long review".into()),
                status: Some("Code Review".into()),
                project_key: "PROJ".into(),
                cycle_secs: Some(100_000),
            }],
            approx_tokens: 120,
        }
    }

    #[tokio::test]
    async fn ask_parses_json_answer_with_citations() {
        let server = httpmock::MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("POST")
                .path(format!("/model/{DEFAULT_MODEL}/converse"))
                .header("Authorization", "Bearer test-key");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                      "output": {
                        "message": {
                          "role": "assistant",
                          "content": [{
                            "text": "{\"text\":\"Review is the bottleneck.\",\"citations\":[\"PROJ-1\",\"bottleneck:Code Review\"]}"
                          }]
                        }
                      }
                    }"#,
                );
        });

        let client = BedrockClient::new_for_test(&server.base_url(), "test-key").unwrap();
        let answer = client
            .ask(&sample_pack(), "Where is the bottleneck?")
            .await
            .unwrap();
        assert_eq!(answer.text, "Review is the bottleneck.");
        assert!(answer.citations.iter().any(|c| c == "PROJ-1"));
        assert!(answer
            .citations
            .iter()
            .any(|c| c == "bottleneck:Code Review"));
        mock.assert();
    }

    #[tokio::test]
    async fn ask_surfaces_api_errors() {
        let server = httpmock::MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("POST")
                .path(format!("/model/{DEFAULT_MODEL}/converse"))
                .header("Authorization", "Bearer bad-key");
            then.status(401).body("unauthorized");
        });

        let client = BedrockClient::new_for_test(&server.base_url(), "bad-key").unwrap();
        let err = client.ask(&sample_pack(), "hi").await.unwrap_err();
        match err {
            BedrockError::Api { status } => assert_eq!(status, 401),
            other => panic!("expected Api, got {other:?}"),
        }
        mock.assert();
    }

    /// Regression: transport failures must not embed the API key in Display/UI strings.
    #[tokio::test]
    async fn failed_request_error_does_not_contain_api_key() {
        const SECRET: &str = "SECRET_KEY_MUST_NOT_LEAK_xyz123";
        let client = BedrockClient::new_for_test("http://127.0.0.1:1", SECRET).unwrap();
        let err = client
            .ask(&sample_pack(), "Where is the bottleneck?")
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            !msg.contains(SECRET),
            "API key leaked into error string: {msg}"
        );
    }

    #[test]
    fn redact_secret_strips_key_from_errors() {
        let leaked = "error sending request for url (https://example/v1) with Bearer SECRET_KEY";
        let cleaned = redact_secret(leaked, "SECRET_KEY");
        assert!(!cleaned.contains("SECRET_KEY"));
        assert!(cleaned.contains("[REDACTED]"));
    }

    #[test]
    fn debug_redacts_api_key() {
        let client = BedrockClient::new_for_test("http://localhost", "super-secret-key").unwrap();
        let debug = format!("{client:?}");
        assert!(!debug.contains("super-secret-key"));
        assert!(debug.contains("[REDACTED]"));
    }
}
