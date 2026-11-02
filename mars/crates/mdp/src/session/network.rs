/**
 * Network, Fetch and Storage domains: cookies as Chromium's network_handler.cc builds them
 * (HttpOnly values included for the trusted client), request headers and user agent overrides,
 * and request interception for navigations: Fetch.enable patterns pause a Page.navigate with a
 * requestPaused event, and continueRequest, fulfillRequest or failRequest resume it through
 * `page.navigate_intercepted`. Subresources never pause; see docs/cdp.md.
 */
use super::{Deferred, Fault, Handler, Session, glob};
use serde_json::{Value, json};

/// Chrome's error when a paused request id is unknown.
const UNKNOWN_REQUEST: &str = "Invalid InterceptionId.";
/// Chrome's error when Fetch is used without enable.
const FETCH_DISABLED: &str = "Fetch domain is not enabled";

/** The Network, Fetch and Storage commands. */
pub(super) const HANDLERS: &[(&str, Handler)] = &[
    ("Network.getCookies", Session::get_cookies),
    ("Network.getAllCookies", Session::get_all_cookies),
    ("Network.setCookie", Session::set_cookie),
    ("Network.setCookies", Session::set_cookies),
    ("Network.deleteCookies", Session::delete_cookies),
    ("Network.clearBrowserCookies", Session::clear_cookies),
    ("Network.setExtraHTTPHeaders", Session::set_extra_headers),
    ("Network.setUserAgentOverride", Session::user_agent_override),
    ("Network.getResponseBody", Session::response_body),
    (
        "Network.getResponseBodyForInterception",
        Session::response_body,
    ),
    ("Fetch.enable", Session::fetch_enable),
    ("Fetch.disable", Session::fetch_disable),
    ("Fetch.continueRequest", Session::continue_request),
    ("Fetch.fulfillRequest", Session::fulfill_request),
    ("Fetch.failRequest", Session::fail_request),
    ("Storage.getCookies", Session::get_all_cookies),
    ("Storage.setCookies", Session::set_cookies),
    ("Storage.clearCookies", Session::clear_cookies),
];

/** One Fetch.enable pattern. */
#[derive(Clone, Debug)]
pub(super) struct Pattern {
    url: String,
    resource_type: Option<String>,
    stage: String,
}

/** A Network.Response for the committed document, as Blink's BuildObjectForResourceResponse. */
pub(super) fn response_object(url: &str, status: u64, mime: &str) -> Value {
    let secure = url.starts_with("https://");
    json!({
        "url": url, "status": status, "statusText": reason(status), "headers": {"content-type": format!("{mime}; charset=utf-8")},
        "mimeType": mime, "charset": "utf-8", "connectionReused": false, "connectionId": 0, "encodedDataLength": 0,
        "securityState": if secure {"secure"} else {"insecure"}, "protocol": "http/1.1", "fromDiskCache": false, "fromServiceWorker": false,
    })
}

/** HTTP reason phrases for the statuses pages commonly carry. */
fn reason(status: u64) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "",
    }
}

/** A Mars cookie entry as Chromium's Network.Cookie. */
fn cookie_object(entry: &Value) -> Value {
    let name = entry["name"].as_str().unwrap_or_default();
    let value = entry["value"].as_str().unwrap_or_default();
    let secure = entry["secure"].as_bool().unwrap_or(false);
    json!({
        "name": name, "value": value, "domain": entry["domain"], "path": entry["path"],
        "expires": entry["expires"].as_u64().map_or(-1.0, |at| at as f64), "size": name.len() + value.len(),
        "httpOnly": entry["http_only"], "secure": secure, "session": entry["expires"].is_null(),
        "sameSite": entry["same_site"], "priority": "Medium", "sameParty": false,
        "sourceScheme": if secure {"Secure"} else {"NonSecure"}, "sourcePort": if secure {443} else {80},
    })
}

/** A Network.CookieParam as the runtime takes it. */
fn cookie_param(param: &Value) -> Value {
    let mut cookie = json!({"name":param["name"],"value":param["value"]});
    for (from, to) in [
        ("url", "url"),
        ("domain", "domain"),
        ("path", "path"),
        ("sameSite", "same_site"),
    ] {
        if let Some(text) = param[from].as_str() {
            cookie[to] = json!(text);
        }
    }
    cookie["secure"] = json!(param["secure"].as_bool().unwrap_or(false));
    cookie["http_only"] = json!(param["httpOnly"].as_bool().unwrap_or(false));
    if let Some(expires) = param["expires"].as_f64() {
        cookie["expires"] = json!(expires);
    }
    cookie
}

impl Session {
    /** The tab a browser-wide cookie command works through: the session's, else one this lane
    opened (and so controls), else any it has seen. */
    fn cookie_tab(&mut self, session: Option<&str>) -> Result<String, Fault> {
        if let Ok(tab) = self.tab(session) {
            return Ok(tab);
        }
        let owned = self
            .targets
            .iter()
            .find(|(_, target)| target.owned)
            .or_else(|| self.targets.iter().next())
            .map(|(tab, _)| tab.clone());
        if let Some(tab) = owned {
            return Ok(tab);
        }
        let tabs = self.call("tab.list", json!({}))?;
        tabs["tabs"][0]["tab_id"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Fault::server("No target is attached."))
    }

    /** Network.getCookies: the cookies the page's URL (or the given URLs) would receive. */
    fn get_cookies(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.cookie_tab(session)?;
        let urls = match params["urls"].as_array() {
            Some(urls) if !urls.is_empty() => urls.clone(),
            _ => vec![json!(self.current_url(&tab))],
        };
        let listed = self.call_tab(&tab, "page.cookies_list", json!({"urls":urls}))?;
        Ok(json!({"cookies":cookies(&listed)}))
    }

    /** Network.getAllCookies and Storage.getCookies: every cookie in the jar. */
    fn get_all_cookies(&mut self, session: Option<&str>, _: &Value) -> Result<Value, Fault> {
        let tab = self.cookie_tab(session)?;
        let listed = self.call_tab(&tab, "page.cookies_list", json!({}))?;
        Ok(json!({"cookies":cookies(&listed)}))
    }

    /** Network.setCookie. */
    fn set_cookie(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.cookie_tab(session)?;
        self.mutate(
            &tab,
            "page.cookies_set",
            json!({"cookies":[cookie_param(params)]}),
        )?;
        Ok(json!({"success":true}))
    }

    /** Network.setCookies and Storage.setCookies. */
    fn set_cookies(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.cookie_tab(session)?;
        let list: Vec<Value> = params["cookies"]
            .as_array()
            .map(|cookies| cookies.iter().map(cookie_param).collect())
            .unwrap_or_default();
        self.mutate(&tab, "page.cookies_set", json!({"cookies":list}))?;
        Ok(json!({}))
    }

    /** Network.deleteCookies. */
    fn delete_cookies(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.cookie_tab(session)?;
        let mut request = json!({"name":params["name"]});
        for field in ["url", "domain", "path"] {
            if let Some(text) = params[field].as_str() {
                request[field] = json!(text);
            }
        }
        self.mutate(&tab, "page.cookies_delete", request)?;
        Ok(json!({}))
    }

    /** Network.clearBrowserCookies and Storage.clearCookies. */
    fn clear_cookies(&mut self, session: Option<&str>, _: &Value) -> Result<Value, Fault> {
        let tab = self.cookie_tab(session)?;
        self.mutate(&tab, "page.cookies_clear", json!({}))?;
        Ok(json!({}))
    }

    /** Network.setExtraHTTPHeaders: validated and recorded; Relay shapes requests itself. */
    fn set_extra_headers(&mut self, _: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let Some(headers) = params["headers"].as_object() else {
            return Err(Fault::invalid("Invalid header value, string expected"));
        };
        let mut accepted = std::collections::BTreeMap::new();
        for (name, value) in headers {
            let Some(value) = value.as_str() else {
                return Err(Fault::invalid("Invalid header value, string expected"));
            };
            if name.is_empty() || name.chars().any(|c| c.is_whitespace() || c == ':') {
                return Err(Fault::invalid("Invalid header name"));
            }
            if value.chars().any(|c| c == '\r' || c == '\n') {
                return Err(Fault::invalid("Invalid header value"));
            }
            accepted.insert(name.clone(), value.to_owned());
        }
        self.extra_headers = accepted;
        Ok(json!({}))
    }

    /** Network.setUserAgentOverride and Emulation.setUserAgentOverride. */
    pub(super) fn user_agent_override(
        &mut self,
        session: Option<&str>,
        params: &Value,
    ) -> Result<Value, Fault> {
        let user_agent = params["userAgent"].as_str().unwrap_or_default();
        if user_agent.chars().any(|c| c == '\r' || c == '\n') {
            return Err(Fault::invalid("Invalid characters found in userAgent"));
        }
        let tab = self.cookie_tab(session)?;
        let mut request = json!({});
        if !user_agent.is_empty() {
            request["user_agent"] = json!(user_agent);
        }
        if let Some(platform) = params["platform"].as_str() {
            request["platform"] = json!(platform);
        }
        if let Some(languages) = params["acceptLanguage"].as_str() {
            let tags: Vec<&str> = languages
                .split(',')
                .map(|tag| tag.split(';').next().unwrap_or_default().trim())
                .filter(|tag| !tag.is_empty())
                .collect();
            request["languages"] = json!(tags);
        }
        self.mutate(&tab, "page.emulate", request)?;
        Ok(json!({}))
    }

    /** Network.getResponseBody: the committed document, serialized; Mars keeps no raw bodies. */
    fn response_body(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let loader = self.targets.get(&tab).map_or(0, |target| target.loader);
        if params["requestId"].as_str() != Some(&super::page::loader_id(loader)) {
            return Err(Fault::server("No resource with given identifier found"));
        }
        let nodes = self.call_tab(&tab, "page.nodes", json!({}))?;
        let html = self.call_tab(
            &tab,
            "page.outer_html",
            json!({"node":nodes["virtual"]["html"]}),
        )?;
        Ok(json!({"body":html["outer_html"],"base64Encoded":false}))
    }

    /** Fetch.enable: remember the patterns; requests matching them pause from now on. */
    fn fetch_enable(&mut self, _: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let patterns = match params["patterns"].as_array() {
            Some(list) => list
                .iter()
                .map(|pattern| Pattern {
                    url: pattern["urlPattern"].as_str().unwrap_or("*").to_owned(),
                    resource_type: pattern["resourceType"].as_str().map(str::to_owned),
                    stage: pattern["requestStage"]
                        .as_str()
                        .unwrap_or("Request")
                        .to_owned(),
                })
                .collect(),
            None => vec![Pattern {
                url: "*".into(),
                resource_type: None,
                stage: "Request".into(),
            }],
        };
        self.fetch = Some(patterns);
        Ok(json!({}))
    }

    /** Fetch.disable: nothing pauses any more; navigations still waiting continue as they were. */
    fn fetch_disable(&mut self, _: Option<&str>, _: &Value) -> Result<Value, Fault> {
        self.fetch = None;
        let waiting = std::mem::take(&mut self.deferred);
        for deferred in waiting {
            self.resume(deferred, json!({}))?;
        }
        Ok(json!({}))
    }

    /** Whether a navigation request matches the enabled patterns at the request stage. */
    fn paused_by_fetch(&self, url: &str) -> bool {
        self.fetch.as_ref().is_some_and(|patterns| {
            patterns.iter().any(|pattern| {
                pattern.stage == "Request"
                    && pattern
                        .resource_type
                        .as_deref()
                        .is_none_or(|kind| kind == "Document")
                    && glob(&pattern.url, url)
            })
        })
    }

    /**
     * Pause a navigation when Fetch asked for it: the client gets requestPaused and the
     * Page.navigate answer waits for its decision.
     */
    pub(super) fn pause_navigation(&mut self, tab: &str, url: &str) -> Result<bool, Fault> {
        if !self.paused_by_fetch(url) {
            return Ok(false);
        }
        let (id, session_id) = self
            .current
            .clone()
            .ok_or_else(|| Fault::server("No command in flight."))?;
        self.next_request += 1;
        let request_id = format!("interception-job-{}", self.next_request);
        self.deferred.push(Deferred {
            id,
            session_id,
            tab: tab.to_owned(),
            request_id: request_id.clone(),
            url: url.to_owned(),
        });
        // Chrome announces the request on the Network domain first; clients pair the pause
        // with it through networkId before they decide.
        let network_id = self.announce_request(tab, url);
        self.emit(Some(tab), "Fetch.requestPaused", json!({
            "requestId": request_id, "frameId": tab, "resourceType": "Document", "networkId": network_id,
            "request": {"url": url, "method": "GET", "headers": self.extra_headers, "initialPriority": "VeryHigh", "referrerPolicy": "strict-origin-when-cross-origin"},
        }));
        Ok(true)
    }

    /** The paused navigation a decision names. */
    fn take_deferred(&mut self, params: &Value) -> Result<Deferred, Fault> {
        if self.fetch.is_none() {
            return Err(Fault::server(FETCH_DISABLED));
        }
        let request_id = params["requestId"].as_str().unwrap_or_default();
        let at = self
            .deferred
            .iter()
            .position(|deferred| deferred.request_id == request_id)
            .ok_or_else(|| Fault::invalid(UNKNOWN_REQUEST))?;
        Ok(self.deferred.remove(at))
    }

    /** Resume a paused navigation with the decision and answer the deferred Page.navigate. */
    fn resume(&mut self, deferred: Deferred, mut decision: Value) -> Result<Value, Fault> {
        decision["url"] = json!(deferred.url);
        let answer = self.perform_navigation(&deferred.tab, "page.navigate_intercepted", decision);
        let response = super::response(deferred.id, deferred.session_id, answer);
        self.events.push(response);
        Ok(json!({}))
    }

    /** Fetch.continueRequest: the same URL, or the one the client chose. */
    fn continue_request(&mut self, _: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let deferred = self.take_deferred(params)?;
        let mut decision = json!({});
        if let Some(url) = params["url"].as_str() {
            decision["continue_url"] = json!(url);
        }
        self.resume(deferred, decision)
    }

    /** Fetch.fulfillRequest: the client's response becomes the document. */
    fn fulfill_request(&mut self, _: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let deferred = self.take_deferred(params)?;
        let status = params["responseCode"].as_u64().unwrap_or(200);
        if !(100..1000).contains(&status) {
            return Err(Fault::invalid("Invalid http status code or phrase"));
        }
        let headers: Vec<Value> = params["responseHeaders"]
            .as_array()
            .map(|entries| {
                entries
                    .iter()
                    .map(|entry| json!([entry["name"], entry["value"]]))
                    .collect()
            })
            .unwrap_or_default();
        let mut fulfill = json!({"status":status,"headers":headers});
        if let Some(body) = params["body"].as_str() {
            fulfill["body_base64"] = json!(body);
        }
        self.resume(deferred, json!({"fulfill":fulfill}))
    }

    /** Fetch.failRequest: Chrome's errorReason becomes the navigation's errorText. */
    fn fail_request(&mut self, _: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let deferred = self.take_deferred(params)?;
        let reason = params["errorReason"]
            .as_str()
            .ok_or_else(|| Fault::invalid("Unknown errorReason"))?;
        self.resume(deferred, json!({"fail":reason}))
    }
}

/** The listing's cookies as Network.Cookie objects. */
fn cookies(listed: &Value) -> Vec<Value> {
    listed["cookies"]
        .as_array()
        .map(|cookies| cookies.iter().map(cookie_object).collect())
        .unwrap_or_default()
}
