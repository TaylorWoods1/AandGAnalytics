//! In-memory HTTP fixture doer for sync engine tests.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use ag_jira::{HttpDoer, HttpRequest, HttpResponse, JiraError};

/// Fake Jira Cloud HTTP layer backed by embedded search fixtures (2 pages, 3 issues).
#[derive(Clone)]
pub struct FakeJira {
    inner: Arc<FakeJiraInner>,
}

struct FakeJiraInner {
    search_calls: AtomicUsize,
    last_search_bodies: Mutex<Vec<String>>,
}

impl FakeJira {
    /// Two-page search fixtures (DEMO-1/2 then DEMO-3) plus projects/fields/boards.
    pub fn from_fixtures_two_pages() -> Self {
        Self {
            inner: Arc::new(FakeJiraInner {
                search_calls: AtomicUsize::new(0),
                last_search_bodies: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Number of `POST /rest/api/3/search/jql` calls observed.
    pub fn search_call_count(&self) -> usize {
        self.inner.search_calls.load(Ordering::SeqCst)
    }

    /// Bodies of search requests (JSON text), in order.
    pub fn search_bodies(&self) -> Vec<String> {
        self.inner.last_search_bodies.lock().expect("lock").clone()
    }
}

impl HttpDoer for FakeJira {
    async fn request(&self, req: HttpRequest) -> Result<HttpResponse, JiraError> {
        let path = req.url.split('?').next().unwrap_or(req.url.as_str());

        if path.ends_with("/rest/api/3/project") {
            return ok_json(include_str!("../tests/fixtures/projects.json"));
        }
        if path.ends_with("/rest/api/3/field") {
            return ok_json(include_str!("../../ag_jira/tests/fixtures/fields.json"));
        }
        if path.ends_with("/rest/agile/1.0/board") {
            return ok_json(include_str!("../tests/fixtures/boards.json"));
        }
        if path.contains("/rest/agile/1.0/board/") && path.ends_with("/sprint") {
            return ok_json(include_str!("../tests/fixtures/sprints.json"));
        }
        if path.contains("/rest/agile/1.0/sprint/") && path.ends_with("/issue") {
            return ok_json(include_str!("../tests/fixtures/sprint_issues.json"));
        }
        if path.ends_with("/rest/api/3/search/jql") {
            let body = String::from_utf8(req.body.unwrap_or_default())
                .map_err(|e| JiraError::Http(e.to_string()))?;
            self.inner
                .last_search_bodies
                .lock()
                .expect("lock")
                .push(body.clone());
            self.inner.search_calls.fetch_add(1, Ordering::SeqCst);

            // Cursor resume / second page.
            if body.contains("page2token") {
                return ok_json(include_str!(
                    "../../ag_jira/tests/fixtures/search_page2.json"
                ));
            }
            // Incremental queries: empty final page (watermark already covers fixtures).
            if body.contains("updated >=") {
                return ok_json(r#"{"issues":[],"isLast":true}"#);
            }
            // Full sync first page.
            return ok_json(include_str!(
                "../../ag_jira/tests/fixtures/search_page1.json"
            ));
        }

        Err(JiraError::Api {
            status: 404,
            body: format!("FakeJira: no fixture for {}", req.url),
        })
    }
}

fn ok_json(body: &str) -> Result<HttpResponse, JiraError> {
    Ok(HttpResponse {
        status: 200,
        headers: vec![("content-type".into(), "application/json".into())],
        body: body.as_bytes().to_vec(),
    })
}
