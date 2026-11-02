/**
 * Fetch interception. A new document is controlled by the active worker of the registration
 * matching its URL (Handle Fetch's "match service worker registration"); a controlled page's
 * fetch and XMLHttpRequest requests, and a navigation whose URL a registration matches, become
 * FetchEvents at that worker when it has a fetch handler. respondWith's Response answers the
 * request; no respondWith falls back to the network; a rejected or invalid response is a network
 * error. The worker runs synchronously until it answers or FETCH_WAIT passes.
 */
use super::Runtime;
use crate::network::WebResponse;
use crate::protocol::EngineResult;
use crate::runtime::page::Page;
use crate::v9::{HostRequest, HostResponse};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/** How long a request waits for its service worker's answer before going to the network. */
const FETCH_WAIT: Duration = Duration::from_secs(10);

/** The storage key of a document at `url` under top-level `top_level`. */
fn key_for(url: &str, top_level: &str) -> Option<String> {
    let url = url::Url::parse(url).ok()?;
    let top = url::Url::parse(top_level).ok()?;
    Some(format!(
        "{} {}",
        url.origin().ascii_serialization(),
        crate::network::partition_site(&top)?
    ))
}

/** What a FetchEvent's outcome means for the request. */
enum Outcome {
    /** No respondWith: the request goes to the network. */
    Network,
    /** A network error. */
    Error,
    /** respondWith's response: its record and body. */
    Response(Value, Vec<u8>),
}

impl Runtime {
    /** Choose and record a new document's controller; the `{controller}` state for its bootstrap. */
    pub(in crate::runtime) fn assign_controller(
        &mut self,
        identity: &str,
        url: &str,
        top_level: &str,
    ) -> Option<String> {
        let key = key_for(url, top_level)?;
        let active = self
            .platform
            .workers
            .registry
            .matching(&key, url)?
            .active
            .as_ref()?;
        let record = active.record();
        let id = active.id;
        self.platform
            .workers
            .live
            .controllers
            .insert(identity.into(), id);
        Some(json!({ "controller": record }).to_string())
    }

    /** The registration and version that would handle a fetch event for `version`, when it has a fetch handler. */
    fn fetch_handler(&mut self, version: u64) -> Option<u64> {
        let registration = self.platform.workers.registry.holding(version)?;
        let active = registration
            .active
            .as_ref()
            .filter(|v| v.id == version && v.has_fetch)?;
        let _ = active;
        Some(registration.id)
    }

    /** Dispatch one FetchEvent and wait for its outcome; None on timeout. */
    fn dispatch_fetch(
        &mut self,
        version: u64,
        event: Value,
        until: Instant,
    ) -> EngineResult<Option<Outcome>> {
        let Some(registration) = self.fetch_handler(version) else {
            return Ok(Some(Outcome::Network));
        };
        self.start_service_worker(registration, version)?;
        let Some(id) = self.queue_service_worker_event(version, event, true) else {
            return Ok(None);
        };
        let deadline = until.max(Instant::now() + FETCH_WAIT);
        self.run_service_worker_until(version, deadline, |running| {
            running.outcomes.contains_key(&id)
        })?;
        let Some((answer, bytes)) = self.take_outcome(version, id) else {
            return Ok(None);
        };
        Ok(Some(if answer["fallback"] == true {
            Outcome::Network
        } else if answer["response"].is_object() {
            Outcome::Response(answer["response"].clone(), bytes)
        } else {
            Outcome::Error
        }))
    }

    /** A controlled page's script request through its worker; None sends it to the network. */
    pub(in crate::runtime) fn service_worker_fetch(
        &mut self,
        page: &Page,
        request: &HostRequest,
        until: Instant,
    ) -> EngineResult<Option<HostResponse>> {
        let Some(version) = self
            .platform
            .workers
            .live
            .controllers
            .get(&page.script_identity)
            .copied()
        else {
            return Ok(None);
        };
        if !request.url.starts_with("http://") && !request.url.starts_with("https://") {
            return Ok(None);
        }
        let event = json!({"kind":"fetch","url":request.url,"method":request.method,"headers":request.headers,
            "body":request.body,"mode":if request.mode.is_empty() {"cors"} else {request.mode.as_str()},
            "credentials":request.credentials,"destination":"","clientId":page.script_identity});
        Ok(match self.dispatch_fetch(version, event, until)? {
            None | Some(Outcome::Network) => None,
            Some(Outcome::Error) => Some(HostResponse {
                id: request.id,
                error: Some("Failed to fetch".into()),
                ..HostResponse::default()
            }),
            Some(Outcome::Response(record, bytes)) => Some(host_response(request, &record, bytes)),
        })
    }

    /** A top-level GET navigation through the worker its URL's registration has; None goes to the network. */
    pub(in crate::runtime) fn service_worker_navigation(
        &mut self,
        url: &str,
        until: Instant,
    ) -> EngineResult<Option<WebResponse>> {
        let Some(key) = key_for(url, url) else {
            return Ok(None);
        };
        let Some(registration) = self.platform.workers.registry.matching(&key, url) else {
            return Ok(None);
        };
        let Some(version) = registration.active.as_ref().map(|v| v.id) else {
            return Ok(None);
        };
        let preload = registration
            .preload_enabled
            .then(|| registration.preload_header.clone());
        let mut event = json!({"kind":"fetch","url":url,"method":"GET","headers":[["accept","text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"]],
            "mode":"navigate","credentials":"include","destination":"document","redirect":"manual","clientId":"","resultingClientId":""});
        if let Some(header) = preload {
            event["preload"] = self.navigation_preload(url, &header, until)?;
        }
        Ok(match self.dispatch_fetch(version, event, until)? {
            Some(Outcome::Response(record, bytes)) if navigable(&record) => {
                Some(web_response(url, &record, bytes))
            }
            _ => None,
        })
    }
}

impl Runtime {
    /** Navigation preload: the navigation's own network response, fetched with the
    Service-Worker-Navigation-Preload header, for `event.preloadResponse`; null when it failed. */
    fn navigation_preload(
        &mut self,
        url: &str,
        header: &str,
        until: Instant,
    ) -> EngineResult<Value> {
        let request = crate::network::WebRequest {
            url: url.into(),
            headers: vec![(PRELOAD_HEADER.into(), header.into())],
            ..crate::network::WebRequest::default()
        };
        let deadline = until.max(Instant::now() + FETCH_WAIT);
        Ok(match self.acquire(request, deadline, None, true, true) {
            Ok(acquired) => {
                let response = acquired.response;
                let content_type = match &response.charset {
                    Some(charset) => format!("{}; charset={charset}", response.mime),
                    None => response.mime.clone(),
                };
                json!({"url":response.url,"status":response.status,"headers":[["content-type",content_type]],"body":response.source})
            }
            Err(error) if crate::runtime::worker_loading::fatal(&error) => return Err(error),
            Err(_) => Value::Null,
        })
    }
}

/** The request header that tells a server a navigation preload is asking. */
const PRELOAD_HEADER: &str = "service-worker-navigation-preload";

/** A worker's response record as the page's HostResponse (an opaque one shows nothing). */
fn host_response(request: &HostRequest, record: &Value, bytes: Vec<u8>) -> HostResponse {
    if record["type"] == "opaque" {
        return HostResponse {
            id: request.id,
            ..HostResponse::default()
        };
    }
    let headers: Vec<(String, String)> =
        serde_json::from_value(record["headers"].clone()).unwrap_or_default();
    let url = record["url"]
        .as_str()
        .filter(|url| !url.is_empty())
        .unwrap_or(&request.url)
        .to_string();
    HostResponse {
        id: request.id,
        status: u16::try_from(record["status"].as_u64().unwrap_or(200)).unwrap_or(200),
        status_text: record["statusText"].as_str().unwrap_or_default().into(),
        url,
        headers: headers
            .into_iter()
            .map(|(name, value)| (name.to_ascii_lowercase(), value))
            .collect(),
        body: String::new(),
        body_bytes: bytes,
        error: None,
    }
}

/** A worker's response to a navigation as the document response Eye parses. */
/** Whether a worker's answer may become a document. Fetch's "handle fetch" treats an opaque,
opaque-redirect or error response to a navigation as a network error: an opaque body is another
origin's data the page must never read, and committing it as the document would hand it over. */
fn navigable(record: &Value) -> bool {
    !matches!(
        record["type"].as_str(),
        Some("opaque" | "opaqueredirect" | "error")
    )
}

/** The document URL for a worker's navigation answer: the response's own URL only when it is
same-origin with the navigation, so a worker cannot commit a page under another origin's URL. */
fn document_url(url: &str, record: &Value) -> String {
    let same_origin = |candidate: &str| match (url::Url::parse(url), url::Url::parse(candidate)) {
        (Ok(a), Ok(b)) => a.origin() == b.origin(),
        _ => false,
    };
    record["url"]
        .as_str()
        .filter(|candidate| !candidate.is_empty() && same_origin(candidate))
        .unwrap_or(url)
        .into()
}

/** A worker's navigation answer as the response the document loads from. */
fn web_response(url: &str, record: &Value, bytes: Vec<u8>) -> WebResponse {
    let headers: Vec<(String, String)> =
        serde_json::from_value(record["headers"].clone()).unwrap_or_default();
    let content_type = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.to_ascii_lowercase())
        .unwrap_or_else(|| "text/html".into());
    let mime = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    let charset = content_type
        .split(';')
        .filter_map(|part| part.trim().strip_prefix("charset="))
        .next()
        .map(|label| label.trim_matches('"').to_string());
    let framing = ["x-frame-options", "content-security-policy"];
    WebResponse {
        url: document_url(url, record),
        status: u16::try_from(record["status"].as_u64().unwrap_or(200)).unwrap_or(200),
        source: String::from_utf8_lossy(&bytes).into_owned(),
        image_data: false,
        raw_data: false,
        mime,
        location: None,
        set_cookies: Vec::new(),
        charset,
        download: false,
        disposition: None,
        headers: headers
            .into_iter()
            .filter(|(name, _)| framing.contains(&name.to_ascii_lowercase().as_str()))
            .collect(),
    }
}

#[cfg(test)]
mod navigation_tests {
    use super::{document_url, navigable};
    use serde_json::json;

    /** Opaque, opaque-redirect and error answers never become a document. */
    #[test]
    fn opaque_answers_are_not_navigable() {
        for kind in ["opaque", "opaqueredirect", "error"] {
            assert!(!navigable(&json!({"type": kind})), "{kind}");
        }
        assert!(navigable(&json!({"type": "basic"})));
        assert!(navigable(&json!({"type": "cors"})));
    }

    /** A response URL from another origin cannot name the committed document. */
    #[test]
    fn document_url_stays_on_the_navigation_origin() {
        let url = "https://a.example/page";
        let same = json!({"url": "https://a.example/other"});
        let cross = json!({"url": "https://b.example/secret"});
        assert_eq!(document_url(url, &same), "https://a.example/other");
        assert_eq!(document_url(url, &cross), url);
        assert_eq!(document_url(url, &json!({"url": ""})), url);
    }
}
