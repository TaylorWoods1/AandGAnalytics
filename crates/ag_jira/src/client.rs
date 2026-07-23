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
use crate::types::{BoardPage, IssueSearchPage, JiraField, Myself, Project, SprintPage};

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
            let value = HeaderValue::from_str(v).map_err(|e| JiraError::Http(e.to_string()))?;
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
    /// Original site URL (e.g. `https://example.atlassian.net`) for tenant lookup.
    site_url: String,
    /// REST base — either the site URL or `https://api.atlassian.com/ex/jira/{cloudId}`.
    base_url: String,
    email: String,
    api_token: String,
    http: H,
}

impl<H: HttpDoer> fmt::Debug for JiraClient<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JiraClient")
            .field("site_url", &self.site_url)
            .field("base_url", &self.base_url)
            .field("email", &self.email)
            .field("api_token", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl JiraClient<ReqwestHttpDoer> {
    /// Build a production client from stored credentials.
    pub fn new(creds: &JiraCredentials) -> Result<Self, JiraError> {
        let site_url = normalize_base_url(&creds.site_url);
        Ok(Self {
            base_url: site_url.clone(),
            site_url,
            email: creds.email.trim().to_string(),
            api_token: creds.api_token.trim().to_string(),
            http: ReqwestHttpDoer::new()?,
        })
    }

    /// Test helper that points at a mock base URL with basic auth.
    pub fn new_for_test(base_url: &str, email: &str, api_token: &str) -> Self {
        let site_url = normalize_base_url(base_url);
        Self {
            base_url: site_url.clone(),
            site_url,
            email: email.trim().to_string(),
            api_token: api_token.trim().to_string(),
            http: ReqwestHttpDoer::new().expect("reqwest client"),
        }
    }
}

impl<H: HttpDoer> JiraClient<H> {
    /// Inject a custom [`HttpDoer`] (useful for unit tests without a real socket).
    pub fn with_http(creds: &JiraCredentials, http: H) -> Self {
        let site_url = normalize_base_url(&creds.site_url);
        Self {
            base_url: site_url.clone(),
            site_url,
            email: creds.email.trim().to_string(),
            api_token: creds.api_token.trim().to_string(),
            http,
        }
    }

    /// Prefer Atlassian API gateway (`api.atlassian.com/ex/jira/{cloudId}`).
    ///
    /// Required for **API tokens with scopes**. Classic (unscoped) tokens work on both
    /// the site URL and the gateway. No-ops if tenant lookup fails (keeps site URL).
    pub async fn use_atlassian_gateway(&mut self) -> Result<(), JiraError> {
        match fetch_cloud_id(&self.http, &self.site_url).await {
            Ok(cloud_id) => {
                self.base_url = format!("https://api.atlassian.com/ex/jira/{cloud_id}");
                Ok(())
            }
            Err(_) => Ok(()),
        }
    }

    /// Active REST base URL (site or gateway).
    pub fn base_url(&self) -> &str {
        &self.base_url
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

    /// `GET /rest/agile/1.0/board`
    pub async fn list_boards_page(&self, start_at: u64) -> Result<BoardPage, JiraError> {
        let path = format!("/rest/agile/1.0/board?startAt={start_at}&maxResults=50");
        let resp = self.get_json(&path).await?;
        Ok(serde_json::from_slice(&resp)?)
    }

    /// `GET /rest/agile/1.0/board/{boardId}/sprint`
    pub async fn list_board_sprints_page(
        &self,
        board_id: i64,
        start_at: u64,
    ) -> Result<SprintPage, JiraError> {
        let path =
            format!("/rest/agile/1.0/board/{board_id}/sprint?startAt={start_at}&maxResults=50");
        let resp = self.get_json(&path).await?;
        Ok(serde_json::from_slice(&resp)?)
    }

    /// `GET /rest/agile/1.0/sprint/{sprintId}/issue` — issue keys/ids only for linking.
    pub async fn list_sprint_issues_page(
        &self,
        sprint_id: i64,
        start_at: u64,
    ) -> Result<IssueSearchPage, JiraError> {
        let path = format!(
            "/rest/agile/1.0/sprint/{sprint_id}/issue?startAt={start_at}&maxResults=50&fields=id,key"
        );
        let resp = self.get_json(&path).await?;
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
    let token = base64::engine::general_purpose::STANDARD.encode(format!("{email}:{api_token}"));
    format!("Basic {token}")
}

/// Resolve Atlassian `cloudId` from `GET {site}/_edge/tenant_info` (no auth).
async fn fetch_cloud_id<H: HttpDoer>(http: &H, site_url: &str) -> Result<String, JiraError> {
    let url = format!("{}/_edge/tenant_info", normalize_base_url(site_url));
    let req = HttpRequest {
        method: "GET".into(),
        url,
        headers: vec![("Accept".into(), "application/json".into())],
        body: None,
    };
    let resp = http.request(req).await?;
    if !(200..300).contains(&resp.status) {
        return Err(JiraError::Api {
            status: resp.status,
            body: "tenant_info unavailable".into(),
        });
    }
    let value: serde_json::Value = serde_json::from_slice(&resp.body)?;
    value
        .get("cloudId")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| JiraError::Api {
            status: 200,
            body: "tenant_info missing cloudId".into(),
        })
}

fn map_response(resp: HttpResponse) -> Result<Vec<u8>, JiraError> {
    match resp.status {
        200..=299 => Ok(resp.body),
        401 => Err(JiraError::Unauthorized),
        403 => {
            let body = String::from_utf8_lossy(&resp.body);
            if looks_like_ip_allowlist(&body) {
                Err(JiraError::IpNotAllowlisted)
            } else {
                Err(JiraError::Forbidden)
            }
        }
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

fn looks_like_ip_allowlist(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    lower.contains("ip allowlist")
        || lower.contains("ip address is not listed")
        || lower.contains("not listed in the ip allowlist")
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
    async fn myself_ip_allowlist_403_is_classified() {
        let server = httpmock::MockServer::start();
        let mock = server.mock(|when, then| {
            when.method("GET").path("/rest/api/3/myself");
            then.status(403)
                .header("content-type", "application/json")
                .body(
                    r#"{"code":403,"message":"You're unable to access content because your IP address is not listed in the IP allowlist. Contact your admin for help."}"#,
                );
        });

        let client = JiraClient::new_for_test(&server.base_url(), "dev@example.com", "token");
        let err = client.get_myself().await.unwrap_err();
        assert!(
            matches!(err, JiraError::IpNotAllowlisted),
            "expected IpNotAllowlisted, got {err:?}"
        );
        mock.assert();
    }

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
                    !body.as_deref().unwrap_or("").contains("nextPageToken")
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
            then.status(429)
                .header("Retry-After", "2")
                .body("slow down");
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
    async fn use_atlassian_gateway_switches_base_url_from_tenant_info() {
        let server = httpmock::MockServer::start();
        let tenant = server.mock(|when, then| {
            when.method("GET").path("/_edge/tenant_info");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"cloudId":"cloud-123"}"#);
        });

        let mut client = JiraClient::new_for_test(&server.base_url(), "dev@example.com", "token");
        client.use_atlassian_gateway().await.unwrap();
        assert_eq!(
            client.base_url(),
            "https://api.atlassian.com/ex/jira/cloud-123"
        );
        tenant.assert();
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
        assert!(
            !debug.contains("dXNlcjpzZWNyZXQ="),
            "secret leaked: {debug}"
        );
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

    #[tokio::test]
    async fn list_boards_and_sprints_parse_fixtures() {
        let server = httpmock::MockServer::start();
        let boards = server.mock(|when, then| {
            when.method("GET").path("/rest/agile/1.0/board");
            then.status(200)
                .header("content-type", "application/json")
                .body(include_str!("../tests/fixtures/boards.json"));
        });
        let sprints = server.mock(|when, then| {
            when.method("GET").path("/rest/agile/1.0/board/1/sprint");
            then.status(200)
                .header("content-type", "application/json")
                .body(include_str!("../tests/fixtures/sprints.json"));
        });

        let client = JiraClient::new_for_test(&server.base_url(), "dev@example.com", "token");
        let page = client.list_boards_page(0).await.unwrap();
        assert_eq!(page.values.len(), 1);
        assert_eq!(page.values[0].id, 1);

        let sprint_page = client.list_board_sprints_page(1, 0).await.unwrap();
        assert_eq!(sprint_page.values.len(), 1);
        assert_eq!(sprint_page.values[0].id, 10);
        assert_eq!(sprint_page.values[0].name, "Sprint 1");

        boards.assert();
        sprints.assert();
    }
}
