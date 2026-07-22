//! Jira Cloud HTTP client.

use std::fmt;
use std::time::Duration;

use ag_credentials::JiraCredentials;
use base64::Engine;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client as ReqwestClient;
use serde::Serialize;
use serde_json::json;

use crate::error::JiraError;
use crate::types::{IssueSearchPage, JiraField, Myself, Project};

/// Outbound HTTP request used by [`HttpDoer`].
///
/// [`Debug`] redacts `Authorization` header values so secrets never hit logs.
#[derive(Clone)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

impl fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let headers: Vec<(&str, &str)> = self
            .headers
            .iter()
            .map(|(k, v)| {
                if k.eq_ignore_ascii_case("authorization") {
                    (k.as_str(), "[REDACTED]")
                } else {
                    (k.as_str(), v.as_str())
                }
            })
            .collect();
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &headers)
            .field("body_len", &self.body.as_ref().map(|b| b.len()))
            .finish()
    }
}

/// HTTP response returned by [`HttpDoer`].
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Pluggable HTTP transport for [`JiraClient`].
pub trait HttpDoer: Send + Sync {
    fn request(
        &self,
        req: HttpRequest,
    ) -> impl std::future::Future<Output = Result<HttpResponse, JiraError>> + Send;
}

/// Production [`HttpDoer`] backed by `reqwest`.
#[derive(Clone)]
pub struct ReqwestHttpDoer {
    client: ReqwestClient,
}

impl ReqwestHttpDoer {
    pub fn new() -> Result<Self, JiraError> {
        let client = ReqwestClient::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| JiraError::Http(e.to_string()))?;
        Ok(Self { client })
    }
}

impl Default for ReqwestHttpDoer {
    fn default() -> Self {
        Self::new().expect("reqwest client")
    }
}

impl HttpDoer for ReqwestHttpDoer {
    async fn request(&self, req: HttpRequest) -> Result<HttpResponse, JiraError> {
        let method = reqwest::Method::from_bytes(req.method.as_bytes())
            .map_err(|e| JiraError::Http(e.to_string()))?;

        let mut headers = HeaderMap::new();
        for (k, v) in &req.headers {
            let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| JiraError::Http(e.to_string()))?;
            let value =
                HeaderValue::from_str(v).map_err(|e| JiraError::Http(e.to_string()))?;
            headers.insert(name, value);
        }

        let mut builder = self.client.request(method, &req.url).headers(headers);
        if let Some(body) = req.body {
            builder = builder.body(body);
        }

        let response = builder
            .send()
            .await
            .map_err(|e| JiraError::Http(e.to_string()))?;

        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = response
            .bytes()
            .await
            .map_err(|e| JiraError::Http(e.to_string()))?
            .to_vec();

        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}

/// Jira Cloud REST client.
pub struct JiraClient<H: HttpDoer> {
    base_url: String,
    email: String,
    api_token: String,
    http: H,
}

impl<H: HttpDoer> fmt::Debug for JiraClient<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JiraClient")
            .field("base_url", &self.base_url)
            .field("email", &self.email)
            .field("api_token", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl JiraClient<ReqwestHttpDoer> {
    /// Build a production client from stored credentials.
    pub fn new(creds: &JiraCredentials) -> Result<Self, JiraError> {
        Ok(Self {
            base_url: normalize_base_url(&creds.site_url),
            email: creds.email.clone(),
            api_token: creds.api_token.clone(),
            http: ReqwestHttpDoer::new()?,
        })
    }

    /// Test helper that points at a mock base URL with basic auth.
    pub fn new_for_test(base_url: &str, email: &str, api_token: &str) -> Self {
        Self {
            base_url: normalize_base_url(base_url),
            email: email.to_string(),
            api_token: api_token.to_string(),
            http: ReqwestHttpDoer::new().expect("reqwest client"),
        }
    }
}

impl<H: HttpDoer> JiraClient<H> {
    /// Inject a custom [`HttpDoer`] (useful for unit tests without a real socket).
    pub fn with_http(creds: &JiraCredentials, http: H) -> Self {
        Self {
            base_url: normalize_base_url(&creds.site_url),
            email: creds.email.clone(),
            api_token: creds.api_token.clone(),
            http,
        }
    }

    /// `GET /rest/api/3/myself`
    pub async fn get_myself(&self) -> Result<Myself, JiraError> {
        let resp = self.get_json("/rest/api/3/myself").await?;
        Ok(serde_json::from_slice(&resp)?)
    }

    /// `GET /rest/api/3/field`
    pub async fn list_fields(&self) -> Result<Vec<JiraField>, JiraError> {
        let resp = self.get_json("/rest/api/3/field").await?;
        Ok(serde_json::from_slice(&resp)?)
    }

    /// `POST /rest/api/3/search/jql` — one page, cursor via `next_page_token`.
    pub async fn search_issues_page(
        &self,
        jql: &str,
        next_page_token: Option<&str>,
        expand_changelog: bool,
    ) -> Result<IssueSearchPage, JiraError> {
        let mut body = json!({
            "jql": jql,
            "maxResults": 50,
            "fields": ["*all"],
        });
        if expand_changelog {
            body["expand"] = json!("changelog");
        }
        if let Some(token) = next_page_token {
            body["nextPageToken"] = json!(token);
        }

        let resp = self
            .send_json("POST", "/rest/api/3/search/jql", Some(&body))
            .await?;
        Ok(serde_json::from_slice(&resp)?)
    }

    /// `GET /rest/api/3/project`
    pub async fn list_projects(&self) -> Result<Vec<Project>, JiraError> {
        let resp = self.get_json("/rest/api/3/project").await?;
        Ok(serde_json::from_slice(&resp)?)
    }

    async fn get_json(&self, path: &str) -> Result<Vec<u8>, JiraError> {
        self.send_json("GET", path, None::<&()>).await
    }

    async fn send_json<B: Serialize>(
        &self,
        method: &str,
        path: &str,
        body: Option<&B>,
    ) -> Result<Vec<u8>, JiraError> {
        let url = format!("{}{path}", self.base_url);
        let auth = basic_auth_header(&self.email, &self.api_token);

        let mut headers = vec![
            ("Authorization".into(), auth),
            ("Accept".into(), "application/json".into()),
        ];
        let body_bytes = match body {
            Some(b) => {
                headers.push(("Content-Type".into(), "application/json".into()));
                Some(
                    serde_json::to_vec(b)
                        .map_err(|e| JiraError::Http(format!("serialize body: {e}")))?,
                )
            }
            None => None,
        };

        // `HttpRequest: Debug` redacts Authorization — never log headers in cleartext.
        let req = HttpRequest {
            method: method.to_string(),
            url,
            headers,
            body: body_bytes,
        };

        let resp = self.http.request(req).await?;
        map_response(resp)
    }
}

fn normalize_base_url(site_url: &str) -> String {
    site_url.trim().trim_end_matches('/').to_string()
}

fn basic_auth_header(email: &str, api_token: &str) -> String {
    let token = base64::engine::general_purpose::STANDARD
        .encode(format!("{email}:{api_token}"));
    format!("Basic {token}")
}

fn map_response(resp: HttpResponse) -> Result<Vec<u8>, JiraError> {
    match resp.status {
        200..=299 => Ok(resp.body),
        401 => Err(JiraError::Unauthorized),
        403 => Err(JiraError::Forbidden),
        429 => {
            let retry_after_ms = resp
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case("retry-after"))
                .and_then(|(_, v)| parse_retry_after_ms(v));
            Err(JiraError::RateLimited { retry_after_ms })
        }
        status => {
            let body = String::from_utf8_lossy(&resp.body);
            let truncated: String = body.chars().take(512).collect();
            Err(JiraError::Api {
                status,
                body: truncated,
            })
        }
    }
}

fn parse_retry_after_ms(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(secs.saturating_mul(1000));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_myself_parses_fixture() {
        let server = httpmock::MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET").path("/rest/api/3/myself");
            then.status(200)
                .header("content-type", "application/json")
                .body(include_str!("../tests/fixtures/myself.json"));
        });

        let client = JiraClient::new_for_test(&server.base_url(), "dev@example.com", "token");
        let me = client.get_myself().await.unwrap();
        assert_eq!(me.account_id, "abc123");
        mock.assert();
    }

    #[tokio::test]
    async fn search_issues_page_follows_pagination_tokens() {
        let server = httpmock::MockServer::start();

        let page1 = server.mock(|when, then| {
            when.method("POST")
                .path("/rest/api/3/search/jql")
                .body_contains(r#""jql":"order by updated asc"#)
                .matches(|req| {
                    let body = req.body.as_ref().map(|b| String::from_utf8_lossy(b));
                    !body
                        .as_deref()
                        .unwrap_or("")
                        .contains("nextPageToken")
                });
            then.status(200)
                .header("content-type", "application/json")
                .body(include_str!("../tests/fixtures/search_page1.json"));
        });

        let page2 = server.mock(|when, then| {
            when.method("POST")
                .path("/rest/api/3/search/jql")
                .body_contains(r#""nextPageToken":"page2token"#);
            then.status(200)
                .header("content-type", "application/json")
                .body(include_str!("../tests/fixtures/search_page2.json"));
        });

        let client = JiraClient::new_for_test(&server.base_url(), "dev@example.com", "token");

        let first = client
            .search_issues_page("order by updated asc", None, true)
            .await
            .unwrap();
        assert_eq!(first.issues.len(), 2);
        assert_eq!(first.issues[0].key, "DEMO-1");
        assert_eq!(first.next_page_token.as_deref(), Some("page2token"));
        assert_eq!(first.is_last, Some(false));

        let second = client
            .search_issues_page("order by updated asc", Some("page2token"), true)
            .await
            .unwrap();
        assert_eq!(second.issues.len(), 1);
        assert_eq!(second.issues[0].key, "DEMO-3");
        assert!(second.next_page_token.is_none());
        assert_eq!(second.is_last, Some(true));

        page1.assert();
        page2.assert();
    }

    #[tokio::test]
    async fn rate_limited_reads_retry_after() {
        let server = httpmock::MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET").path("/rest/api/3/myself");
            then.status(429).header("Retry-After", "2").body("slow down");
        });

        let client = JiraClient::new_for_test(&server.base_url(), "dev@example.com", "token");
        let err = client.get_myself().await.unwrap_err();
        match err {
            crate::error::JiraError::RateLimited { retry_after_ms } => {
                assert_eq!(retry_after_ms, Some(2000));
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
        mock.assert();
    }

    #[tokio::test]
    async fn http_request_debug_redacts_authorization() {
        let req = HttpRequest {
            method: "GET".into(),
            url: "https://example.atlassian.net/rest/api/3/myself".into(),
            headers: vec![
                ("Authorization".into(), "Basic dXNlcjpzZWNyZXQ=".into()),
                ("Accept".into(), "application/json".into()),
            ],
            body: None,
        };
        let debug = format!("{req:?}");
        assert!(!debug.contains("dXNlcjpzZWNyZXQ="), "secret leaked: {debug}");
        assert!(debug.contains("[REDACTED]"), "expected redaction: {debug}");
        assert!(debug.contains("Accept"));
    }

    #[tokio::test]
    async fn list_fields_parses_fixture() {
        let server = httpmock::MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET").path("/rest/api/3/field");
            then.status(200)
                .header("content-type", "application/json")
                .body(include_str!("../tests/fixtures/fields.json"));
        });

        let client = JiraClient::new_for_test(&server.base_url(), "dev@example.com", "token");
        let fields = client.list_fields().await.unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1].id, "customfield_10016");
        assert_eq!(fields[1].name, "Story Points");
        mock.assert();
    }
}
