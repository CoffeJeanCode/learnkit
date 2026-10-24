//! Web search for the resource-research agent, behind a trait so the pipeline
//! is testable without a network and the provider can be swapped.
//!
//! Only the TITLES, URLs and snippets this returns can ever become a resource
//! (see `domain::resources::ground_selection`): the model picks among hits, it
//! never supplies a link of its own.
//!
//! The one real implementation is Tavily (`POST /search`, bearer key). That
//! request/response shape was written from the provider's documented API and is
//! NOT verified against the live service in this repo's tests: they pin what we
//! send and how we read a response of that shape through a local stand-in
//! server, nothing more.

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;

use crate::domain::resources::{SearchHit, clean_hits};
use crate::error::{AppError, AppResult};

/// Vault id of the search key. Rides the existing provider-key storage under
/// a reserved name (`provider:search:tavily`), so it gets the same encryption.
pub const SEARCH_KEY_ID: &str = "search:tavily";

/// Env var fallback, read only when the vault holds no key (developer convenience).
pub const SEARCH_KEY_ENV: &str = "TAVILY_API_KEY";

const DEFAULT_BASE_URL: &str = "https://api.tavily.com";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[async_trait]
pub trait WebSearch: Send + Sync {
    /// Up to `max_results` cleaned hits for `query`, best first.
    async fn search(&self, query: &str, max_results: usize) -> AppResult<Vec<SearchHit>>;
}

pub struct TavilySearch {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl TavilySearch {
    pub fn new(api_key: String) -> Self {
        Self::with_base_url(api_key, DEFAULT_BASE_URL.to_string())
    }

    /// Points the client elsewhere — tests use a local stand-in server.
    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        let client = reqwest::Client::builder().timeout(REQUEST_TIMEOUT).build().unwrap_or_else(|_| reqwest::Client::new());
        Self { api_key, base_url: base_url.trim_end_matches('/').to_string(), client }
    }
}

/// The JSON body of one search request.
pub(crate) fn tavily_request_body(query: &str, max_results: usize) -> serde_json::Value {
    serde_json::json!({
        "query": query,
        "search_depth": "basic",
        "max_results": max_results.clamp(1, 10),
        "include_answer": false,
        "include_raw_content": false,
    })
}

#[derive(Deserialize)]
struct TavilyResponse {
    #[serde(default)]
    results: Vec<TavilyResult>,
}

#[derive(Deserialize)]
struct TavilyResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content: String,
}

/// Reads a response body into cleaned hits. A body that is not the expected
/// JSON is an error (never "zero results"), so a changed API surfaces instead
/// of silently producing courses with no resources.
pub(crate) fn parse_tavily_response(body: &str) -> AppResult<Vec<SearchHit>> {
    let parsed: TavilyResponse =
        serde_json::from_str(body).map_err(|e| AppError::ProviderConnectionFailed(format!("respuesta del buscador ilegible: {e}")))?;
    Ok(clean_hits(parsed.results.into_iter().map(|r| SearchHit { title: r.title, url: r.url, snippet: r.content }).collect()))
}

/// Maps an HTTP failure to the error the caller should act on: a rejected key
/// is permanent (a retry cannot fix it); rate limits and server errors are
/// transient (the pipeline may retry with backoff).
pub(crate) fn status_error(status: reqwest::StatusCode) -> AppError {
    match status.as_u16() {
        401 | 403 => AppError::ProviderKeyMissing("buscador de recursos (clave rechazada)".to_string()),
        429 => AppError::ProviderConnectionFailed("límite de uso del buscador alcanzado".to_string()),
        code => AppError::ProviderConnectionFailed(format!("el buscador respondió {code}")),
    }
}

#[async_trait]
impl WebSearch for TavilySearch {
    async fn search(&self, query: &str, max_results: usize) -> AppResult<Vec<SearchHit>> {
        let response = self
            .client
            .post(format!("{}/search", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&tavily_request_body(query, max_results))
            .send()
            .await
            // The key travels in a header, never the URL, so this message cannot leak it.
            .map_err(|e| AppError::ProviderConnectionFailed(format!("no pude contactar al buscador: {}", e.without_url())))?;
        let status = response.status();
        if !status.is_success() {
            return Err(status_error(status));
        }
        let body = response.text().await.map_err(|e| AppError::ProviderConnectionFailed(format!("respuesta del buscador cortada: {}", e.without_url())))?;
        parse_tavily_response(&body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn the_request_body_asks_for_a_few_cheap_results() {
        let body = tavily_request_body("instalar ubuntu", 5);
        assert_eq!(body["query"], "instalar ubuntu");
        assert_eq!(body["max_results"], 5);
        assert_eq!(body["search_depth"], "basic", "the cheap depth: this runs once per class");
        assert_eq!(body["include_answer"], false);
        assert_eq!(body["include_raw_content"], false, "full pages are never fetched");
        assert_eq!(tavily_request_body("q", 999)["max_results"], 10, "bounded");
        assert_eq!(tavily_request_body("q", 0)["max_results"], 1);
    }

    #[test]
    fn a_response_becomes_cleaned_hits() {
        let body = r#"{"query":"q","results":[
            {"title":"Guía","url":"https://a.dev/x","content":"Resumen","score":0.9},
            {"title":"Repetida","url":"https://a.dev/x/?utm_source=z","content":"dup","score":0.8},
            {"title":"","url":"https://b.dev","content":"sin título"},
            {"title":"Mala","url":"javascript:alert(1)","content":"nope"}
        ],"response_time":1.2}"#;
        let hits = parse_tavily_response(body).expect("parses");
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].title.as_str(), hits[0].snippet.as_str()), ("Guía", "Resumen"));
    }

    #[test]
    fn an_empty_result_list_is_fine_but_a_changed_api_is_an_error() {
        assert!(parse_tavily_response(r#"{"results":[]}"#).expect("ok").is_empty());
        assert!(parse_tavily_response(r#"{}"#).expect("a missing list means no results").is_empty());
        for bad in ["", "not json", "[1,2]", "<html>502</html>"] {
            assert!(matches!(parse_tavily_response(bad), Err(AppError::ProviderConnectionFailed(_))), "{bad:?} must not read as zero results");
        }
    }

    #[test]
    fn http_failures_map_to_permanent_or_transient_errors() {
        use reqwest::StatusCode;
        assert!(!status_error(StatusCode::UNAUTHORIZED).is_transient(), "a rejected key is permanent");
        assert!(!status_error(StatusCode::FORBIDDEN).is_transient());
        assert!(status_error(StatusCode::TOO_MANY_REQUESTS).is_transient(), "a rate limit may clear");
        assert!(status_error(StatusCode::BAD_GATEWAY).is_transient());
        assert!(matches!(status_error(StatusCode::UNAUTHORIZED), AppError::ProviderKeyMissing(_)));
    }

    /// One-shot stand-in server: returns what it should answer, and records the
    /// raw request it received.
    async fn serve_once(status_line: &'static str, body: &'static str) -> (String, Arc<Mutex<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let seen = Arc::new(Mutex::new(String::new()));
        let seen2 = seen.clone();
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.expect("accept");
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = sock.read(&mut chunk).await.expect("read");
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(head_end) = text.find("\r\n\r\n") {
                    let len = text[..head_end].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok())).unwrap_or(0);
                    if buf.len() >= head_end + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            *seen2.lock().unwrap() = String::from_utf8_lossy(&buf).to_string();
            let reply = format!("HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            sock.write_all(reply.as_bytes()).await.expect("write");
        });
        (format!("http://{addr}"), seen)
    }

    #[tokio::test]
    async fn the_client_sends_a_bearer_key_and_the_query_and_reads_the_hits() {
        let (base, seen) = serve_once("200 OK", r#"{"results":[{"title":"Guía","url":"https://a.dev/x","content":"Resumen"}]}"#).await;
        let search = TavilySearch::with_base_url("tvly-secret".to_string(), base);

        let hits = search.search("instalar ubuntu", 5).await.expect("search ok");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].url, "https://a.dev/x");
        let request = seen.lock().unwrap().clone();
        assert!(request.starts_with("POST /search "), "{request}");
        assert!(request.to_ascii_lowercase().contains("authorization: bearer tvly-secret"), "the key goes in the header: {request}");
        assert!(request.contains("\"query\":\"instalar ubuntu\"") && request.contains("\"max_results\":5"), "{request}");
        assert!(!request.lines().next().unwrap().contains("tvly-secret"), "the key never appears in the URL line");
    }

    #[tokio::test]
    async fn a_rejected_key_is_a_permanent_error_and_never_echoes_the_key() {
        let (base, _) = serve_once("401 Unauthorized", r#"{"detail":"bad key"}"#).await;
        let err = TavilySearch::with_base_url("tvly-secret".to_string(), base).search("q", 3).await.expect_err("rejected");
        assert!(!err.is_transient());
        assert!(!err.to_string().contains("tvly-secret"), "{err}");
    }

    #[tokio::test]
    async fn an_unreachable_server_is_a_transient_error() {
        // Port 1 on localhost: nothing listens there.
        let err = TavilySearch::with_base_url("k".to_string(), "http://127.0.0.1:1".to_string()).search("q", 3).await.expect_err("unreachable");
        assert!(err.is_transient(), "{err}");
    }
}
