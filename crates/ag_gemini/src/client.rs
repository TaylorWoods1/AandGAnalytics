//! Google Gemini `generateContent` HTTP client.

use std::fmt;
use std::time::Duration;

use ag_credentials::GeminiCredentials;
use reqwest::Client as ReqwestClient;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::context::{format_pack_for_prompt, ContextPack};
use crate::error::GeminiError;

/// Default Gemini model for Q&A.
pub const DEFAULT_MODEL: &str = "gemini-2.0-flash";

/// System instruction: ground answers in the local context pack only.
pub const SYSTEM_INSTRUCTION: &str = "You are an analytics assistant for a local Jira engineering intelligence app. \
Answer ONLY using the provided context pack. Cite issue keys (e.g. PROJ-1) and metric names \
(e.g. bottleneck:Code Review, cycle_p50_secs) that support your answer. \
If the context pack does not contain enough information, say you don't know and do not invent data. \
Respond as JSON with keys \"text\" (string) and \"citations\" (array of strings).";

/// Grounded answer from Gemini with citation strings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GeminiAnswer {
    pub text: String,
    pub citations: Vec<String>,
}

/// Gemini generateContent client (base URL injectable for tests).
pub struct GeminiClient {
    api_key: String,
    base_url: String,
    model: String,
    http: ReqwestClient,
}

impl fmt::Debug for GeminiClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GeminiClient")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl GeminiClient {
    /// Production client talking to Google Generative Language API.
    pub fn new(creds: &GeminiCredentials) -> Result<Self, GeminiError> {
        if creds.api_key.trim().is_empty() {
            return Err(GeminiError::MissingApiKey);
        }
        Self::with_base_url(
            creds.api_key.clone(),
            "https://generativelanguage.googleapis.com".into(),
            DEFAULT_MODEL.into(),
        )
    }

    /// Test helper with mock server base URL.
    pub fn new_for_test(base_url: &str, api_key: &str) -> Result<Self, GeminiError> {
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
    ) -> Result<Self, GeminiError> {
        let http = ReqwestClient::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| GeminiError::Http(e.to_string()))?;
        Ok(Self {
            api_key,
            base_url,
            model,
            http,
        })
    }

    /// Ask a question grounded in `pack`. Failures are isolated to AI — callers must not affect dashboards.
    pub async fn ask(
        &self,
        pack: &ContextPack,
        question: &str,
    ) -> Result<GeminiAnswer, GeminiError> {
        let q = question.trim();
        if q.is_empty() {
            return Err(GeminiError::Other("question must not be empty".into()));
        }

        let context = format_pack_for_prompt(pack);
        let user_text =
            format!("Context pack:\n{context}\n\nQuestion: {q}\n\nRespond with JSON only.");

        let url = format!(
            "{}/v1beta/models/{}:generateContent",
            self.base_url, self.model
        );

        let body = json!({
            "systemInstruction": {
                "parts": [{ "text": SYSTEM_INSTRUCTION }]
            },
            "contents": [{
                "role": "user",
                "parts": [{ "text": user_text }]
            }],
            "generationConfig": {
                "temperature": 0.2,
                "responseMimeType": "application/json"
            }
        });

        let resp = self
            .http
            .post(&url)
            .query(&[("key", self.api_key.as_str())])
            .json(&body)
            .send()
            .await
            .map_err(|e| GeminiError::Http(e.to_string()))?;

        let status = resp.status().as_u16();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| GeminiError::Http(e.to_string()))?;

        if !(200..300).contains(&status) {
            return Err(GeminiError::Api { status });
        }

        let envelope: Value =
            serde_json::from_slice(&bytes).map_err(|e| GeminiError::Parse(e.to_string()))?;
        let text = extract_candidate_text(&envelope)?;
        parse_answer(&text)
    }

    /// Lightweight connectivity probe (list models).
    pub async fn probe(&self) -> Result<String, GeminiError> {
        let url = format!("{}/v1beta/models", self.base_url);
        let resp = self
            .http
            .get(&url)
            .query(&[("key", self.api_key.as_str())])
            .send()
            .await
            .map_err(|e| GeminiError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        if (200..300).contains(&status) {
            Ok("gemini models reachable".into())
        } else {
            Err(GeminiError::Api { status })
        }
    }
}

fn extract_candidate_text(envelope: &Value) -> Result<String, GeminiError> {
    let text = envelope
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| GeminiError::Parse("missing candidates[0].content.parts[0].text".into()))?;
    Ok(text.to_string())
}

fn parse_answer(raw: &str) -> Result<GeminiAnswer, GeminiError> {
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
        return Ok(GeminiAnswer { text, citations });
    }

    // Fallback: plain text with heuristic citations (ISSUE-123 / bottleneck:Name).
    let citations = extract_citations_heuristic(trimmed);
    Ok(GeminiAnswer {
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
                .path(format!("/v1beta/models/{DEFAULT_MODEL}:generateContent"));
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
                      "candidates": [{
                        "content": {
                          "parts": [{
                            "text": "{\"text\":\"Review is the bottleneck.\",\"citations\":[\"PROJ-1\",\"bottleneck:Code Review\"]}"
                          }]
                        }
                      }]
                    }"#,
                );
        });

        let client = GeminiClient::new_for_test(&server.base_url(), "test-key").unwrap();
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
                .path(format!("/v1beta/models/{DEFAULT_MODEL}:generateContent"));
            then.status(401).body("unauthorized");
        });

        let client = GeminiClient::new_for_test(&server.base_url(), "bad-key").unwrap();
        let err = client.ask(&sample_pack(), "hi").await.unwrap_err();
        match err {
            GeminiError::Api { status } => assert_eq!(status, 401),
            other => panic!("expected Api, got {other:?}"),
        }
        mock.assert();
    }

    #[test]
    fn debug_redacts_api_key() {
        let client = GeminiClient::new_for_test("http://localhost", "super-secret-key").unwrap();
        let debug = format!("{client:?}");
        assert!(!debug.contains("super-secret-key"));
        assert!(debug.contains("[REDACTED]"));
    }
}
