/**
 * Request interception for the Fetch domain: a navigation the CDP client paused resumes here
 * with the client's decision (continue, possibly to another URL; fulfill with a synthetic
 * response; fail), applied at the one place every outbound request passes, `acquire_with`.
 * The decision is scoped to the navigation call that installs it, so another lane's request
 * interleaved on the runtime queue can never pick it up.
 */
use super::super::supervisor::Runtime;
use crate::constants::HTML_BYTES;
use crate::network::{WebRequest, WebResponse};
use crate::protocol::{EngineError, EngineResult, parameters};
use serde::Deserialize;
use serde_json::{Value, json};
use std::cell::RefCell;

/// Status text Chrome's Fetch.fulfillRequest defaults to for a code without a phrase.
const DEFAULT_PHRASE: &str = "OK";

/** A synthetic response the client supplied for the paused request. */
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Fulfill {
    status: u16,
    #[serde(default)]
    headers: Vec<(String, String)>,
    #[serde(default)]
    body_base64: Option<String>,
}

/** page.navigate_intercepted: the paused navigation and the client's decision. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Intercepted {
    /// The URL the client was told about.
    url: String,
    /// Fetch.continueRequest with a different URL.
    #[serde(default)]
    continue_url: Option<String>,
    /// Fetch.fulfillRequest.
    #[serde(default)]
    fulfill: Option<Fulfill>,
    /// Fetch.failRequest: Chrome's `Network.ErrorReason` name.
    #[serde(default)]
    fail: Option<String>,
}

/** The decision waiting for the next top-level request of `url`. */
struct Override {
    url: String,
    continue_url: Option<String>,
    fulfill: Option<Fulfill>,
    fail: Option<String>,
}

thread_local! {
    /** One decision, alive only inside the navigation call that installed it. The runtime runs on
    one thread, so the slot is that call's own; nothing else reads it. */
    // ponytail: a Runtime field would be the honest home; supervisor.rs is another agent's file
    // right now, so the decision lives beside the runtime until a field can be added.
    static OVERRIDE: RefCell<Option<Override>> = const { RefCell::new(None) };
}

/**
 * Called by `acquire_with` for the first hop of a top-level request. Rewrites the URL for a
 * continued request, answers a fulfilled one without the network, or fails a failed one. A
 * request for another URL leaves the decision in place.
 */
pub(in crate::runtime) fn intercept(request: &mut WebRequest) -> Option<EngineResult<WebResponse>> {
    OVERRIDE.with_borrow_mut(|slot| {
        let pending = slot.as_ref()?;
        if pending.url != request.url {
            return None;
        }
        let decision = slot.take()?;
        if let Some(reason) = decision.fail {
            return Some(Err(failed(&reason)));
        }
        if let Some(fulfill) = decision.fulfill {
            return Some(synthetic(&request.url, fulfill));
        }
        if let Some(url) = decision.continue_url {
            request.url = url;
        }
        None
    })
}

/** The error a failed request surfaces; the front door reports it as Chrome's `errorText`. */
fn failed(reason: &str) -> EngineError {
    EngineError::new(
        "navigation_failed",
        format!(
            "net::ERR_{}",
            reason.trim_start_matches("ERR_").to_ascii_uppercase()
        ),
        "The CDP client failed this request.",
    )
}

/** A fulfilled response, shaped as Relay would have returned it. */
fn synthetic(url: &str, fulfill: Fulfill) -> EngineResult<WebResponse> {
    let headers: Vec<(String, String)> = fulfill
        .headers
        .iter()
        .map(|(name, value)| (name.to_ascii_lowercase(), value.clone()))
        .collect();
    let header = |wanted: &str| {
        headers
            .iter()
            .find(|(name, _)| name == wanted)
            .map(|(_, value)| value.clone())
    };
    let mime = header("content-type")
        .map(|value| {
            value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        })
        .filter(|mime| !mime.is_empty())
        .unwrap_or_else(|| "text/html".into());
    let body = match &fulfill.body_base64 {
        Some(encoded) => super::super::images::decode_base64_within(encoded, HTML_BYTES)?,
        None => Vec::new(),
    };
    let source = String::from_utf8(body)
        .map_err(|_| EngineError::unsupported("A fulfilled document must be UTF-8 text."))?;
    Ok(WebResponse {
        url: url.into(),
        status: fulfill.status,
        source,
        image_data: false,
        raw_data: false,
        mime,
        location: header("location"),
        set_cookies: headers
            .iter()
            .filter(|(name, _)| name == "set-cookie")
            .map(|(_, value)| value.clone())
            .collect(),
        charset: None,
        download: false,
        disposition: None,
        headers,
    })
}

impl Runtime {
    /**
     * page.navigate_intercepted: the ordinary navigation, with the client's decision applied to
     * its first request. The decision is removed again whatever happens, so it cannot leak into a
     * later navigation.
     */
    pub(in crate::runtime) fn navigate_intercepted(
        &mut self,
        params: Value,
    ) -> EngineResult<Value> {
        let input: Intercepted = parameters(params)?;
        OVERRIDE.with_borrow_mut(|slot| {
            *slot = Some(Override {
                url: input.url.clone(),
                continue_url: input.continue_url,
                fulfill: input.fulfill,
                fail: input.fail,
            });
        });
        let result = self.navigate(json!({"url":input.url}));
        OVERRIDE.with_borrow_mut(|slot| *slot = None);
        let _ = DEFAULT_PHRASE;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A fulfilled decision answers exactly its URL once; other URLs pass untouched. */
    #[test]
    fn fulfilled_decision_answers_its_url_once() {
        OVERRIDE.with_borrow_mut(|slot| {
            *slot = Some(Override {
                url: "https://a.example/".into(),
                continue_url: None,
                fulfill: Some(Fulfill {
                    status: 200,
                    headers: vec![("Content-Type".into(), "text/html; charset=utf-8".into())],
                    body_base64: Some("PGgxPkhpPC9oMT4=".into()),
                }),
                fail: None,
            });
        });
        let mut other = WebRequest::get("https://b.example/");
        assert!(intercept(&mut other).is_none());
        let mut request = WebRequest::get("https://a.example/");
        let response = intercept(&mut request).unwrap().unwrap();
        assert_eq!(response.source, "<h1>Hi</h1>");
        assert_eq!(response.mime, "text/html");
        assert!(intercept(&mut request).is_none());
    }

    /** A failed decision is the client's net error; a continued one rewrites the URL. */
    #[test]
    fn failed_and_continued_decisions_apply() {
        OVERRIDE.with_borrow_mut(|slot| {
            *slot = Some(Override {
                url: "https://a.example/".into(),
                continue_url: None,
                fulfill: None,
                fail: Some("Failed".into()),
            });
        });
        let mut request = WebRequest::get("https://a.example/");
        let error = intercept(&mut request).unwrap().unwrap_err();
        assert_eq!(error.message, "net::ERR_FAILED");
        OVERRIDE.with_borrow_mut(|slot| {
            *slot = Some(Override {
                url: "https://a.example/".into(),
                continue_url: Some("https://a.example/else".into()),
                fulfill: None,
                fail: None,
            });
        });
        assert!(intercept(&mut request).is_none());
        assert_eq!(request.url, "https://a.example/else");
    }
}
