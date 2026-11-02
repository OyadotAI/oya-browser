/**
 * Cookie operations for the Network and Storage domains: list, set, delete and clear, over the
 * runtime's one jar, persisted through the profile exactly as server cookies are. The CDP
 * client is trusted the way Chrome trusts its DevTools client, so HttpOnly values are listed;
 * page scripts still never see them.
 */
use super::super::supervisor::Runtime;
use crate::network::CookieEntry;
use crate::protocol::{EngineError, EngineResult, parameters};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

/** page.cookies_list: optionally only the cookies these URLs would receive. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct List {
    #[serde(default)]
    urls: Option<Vec<String>>,
}

/** One cookie to set: Chromium's `Network.CookieParam`. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Param {
    name: String,
    value: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    secure: bool,
    #[serde(default)]
    http_only: bool,
    #[serde(default)]
    same_site: Option<String>,
    /// Seconds since the Unix epoch; absent or negative means a session cookie.
    #[serde(default)]
    expires: Option<f64>,
}

/** page.cookies_set. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Set {
    cookies: Vec<Param>,
}

/** page.cookies_delete: Chromium needs a URL or a domain beside the name. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Delete {
    name: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    domain: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

/** Cookies one listing returns at most; the jar itself is bounded lower. */
const LISTED: usize = 4096;

impl Runtime {
    /** page.cookies_list: every cookie, or those sent to the given URLs. */
    pub(in crate::runtime) fn cookies_list(&mut self, params: Value) -> EngineResult<Value> {
        let input: List = parameters(params)?;
        let urls: Vec<Url> = input
            .urls
            .unwrap_or_default()
            .iter()
            .filter_map(|url| Url::parse(url).ok())
            .collect();
        let entries = self.cookies.entries(SystemTime::now());
        let cookies: Vec<Value> = entries
            .iter()
            .filter(|cookie| urls.is_empty() || urls.iter().any(|url| sent_to(cookie, url)))
            .take(LISTED)
            .map(|cookie| serde_json::to_value(cookie).unwrap_or_default())
            .collect();
        Ok(json!({"cookies":cookies}))
    }

    /** page.cookies_set: each cookie as a Set-Cookie field received from its URL. */
    pub(in crate::runtime) fn cookies_set(&mut self, params: Value) -> EngineResult<Value> {
        let input: Set = parameters(params)?;
        let mut fields = Vec::with_capacity(input.cookies.len());
        for cookie in &input.cookies {
            fields.push(set_cookie_field(cookie)?);
        }
        let mut refused = Vec::new();
        self.commit_jar(|jar| {
            let now = SystemTime::now();
            for (url, field) in &fields {
                refused.extend(jar.receive(url, std::slice::from_ref(field), now));
            }
            Vec::new()
        })?;
        if !refused.is_empty() {
            return Err(EngineError::invalid("Invalid cookie fields"));
        }
        Ok(json!({"set":fields.len()}))
    }

    /** page.cookies_delete: by name and domain (or the URL's host), optionally by path. */
    pub(in crate::runtime) fn cookies_delete(&mut self, params: Value) -> EngineResult<Value> {
        let input: Delete = parameters(params)?;
        let domain = match (&input.domain, &input.url) {
            (Some(domain), _) if !domain.is_empty() => domain.clone(),
            (_, Some(url)) => Url::parse(url)
                .ok()
                .filter(|url| ["http", "https"].contains(&url.scheme()))
                .and_then(|url| url.host_str().map(str::to_owned))
                .ok_or_else(|| {
                    EngineError::invalid("An http or https url URL must be specified")
                })?,
            _ => {
                return Err(EngineError::invalid(
                    "At least one of the url and domain needs to be specified",
                ));
            }
        };
        let mut removed = 0;
        self.commit_jar(|jar| {
            removed = jar.remove(&input.name, &domain, input.path.as_deref());
            Vec::new()
        })?;
        Ok(json!({"removed":removed}))
    }

    /** page.cookies_clear: the whole jar, persisted empty. */
    pub(in crate::runtime) fn cookies_clear(&mut self, params: Value) -> EngineResult<Value> {
        let _: super::super::page::Empty = parameters(params)?;
        self.commit_jar(|jar| {
            jar.clear();
            Vec::new()
        })?;
        Ok(json!({"cleared":true}))
    }
}

/** Whether a cookie would go on a request to `url`: domain, path and Secure, as the jar sends. */
fn sent_to(cookie: &CookieEntry, url: &Url) -> bool {
    let host = url.host_str().unwrap_or_default();
    let domain = cookie.domain.trim_start_matches('.');
    let domain_ok = if cookie.domain.starts_with('.') {
        host == domain || host.ends_with(&format!(".{domain}"))
    } else {
        host == domain
    };
    let path = url.path();
    let path_ok = path == cookie.path
        || (path.starts_with(&cookie.path)
            && (cookie.path.ends_with('/') || path[cookie.path.len()..].starts_with('/')));
    domain_ok && path_ok && (!cookie.secure || url.scheme() == "https")
}

/**
 * The URL a cookie param is received from and its Set-Cookie field, following Chromium's
 * `MakeCookieFromProtocolValues`: a dotted domain becomes a Domain attribute; a bare domain
 * names the host; `https` forces Secure; a negative or missing expiry means a session cookie.
 */
fn set_cookie_field(param: &Param) -> EngineResult<(Url, String)> {
    let mut domain = param.domain.clone().unwrap_or_default();
    let mut secure = param.secure;
    if param.url.is_none() && domain.is_empty() {
        return Err(EngineError::invalid(
            "At least one of the url or domain needs to be specified",
        ));
    }
    let mut host = domain.trim_start_matches('.').to_owned();
    if let Some(url) = &param.url {
        let parsed = Url::parse(url).map_err(|_| EngineError::invalid("Invalid cookie fields"))?;
        if !["http", "https"].contains(&parsed.scheme()) {
            return Err(EngineError::invalid("URL must have scheme http or https"));
        }
        secure |= parsed.scheme() == "https";
        if host.is_empty() {
            host = parsed.host_str().unwrap_or_default().to_owned();
        }
    }
    if !domain.starts_with('.') {
        domain.clear();
    }
    let scheme = if secure { "https" } else { "http" };
    let path = param.path.clone().unwrap_or_else(|| "/".into());
    let source = Url::parse(&format!("{scheme}://{host}{path}"))
        .map_err(|_| EngineError::invalid("Invalid cookie fields"))?;
    let mut field = format!("{}={}; Path={path}", param.name, param.value);
    if !domain.is_empty() {
        field.push_str(&format!("; Domain={domain}"));
    }
    if secure {
        field.push_str("; Secure");
    }
    if param.http_only {
        field.push_str("; HttpOnly");
    }
    if let Some(same_site) = &param.same_site {
        field.push_str(&format!("; SameSite={same_site}"));
    }
    if let Some(expires) = param.expires.filter(|at| *at >= 0.0) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |since| since.as_secs_f64());
        field.push_str(&format!(
            "; Max-Age={}",
            (expires - now).ceil().max(0.0) as i64
        ));
    }
    Ok((source, field))
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Cookie params become the Set-Cookie fields Chromium would build from them. */
    #[test]
    fn params_become_set_cookie_fields() {
        let (url, field) = set_cookie_field(&Param {
            name: "sid".into(),
            value: "1".into(),
            url: Some("https://shop.example/account".into()),
            domain: Some(".example".into()),
            path: None,
            secure: false,
            http_only: true,
            same_site: Some("Lax".into()),
            expires: None,
        })
        .unwrap();
        assert_eq!(url.as_str(), "https://example/");
        assert_eq!(
            field,
            "sid=1; Path=/; Domain=.example; Secure; HttpOnly; SameSite=Lax"
        );
        assert!(
            set_cookie_field(&Param {
                name: "a".into(),
                value: "b".into(),
                url: None,
                domain: None,
                path: None,
                secure: false,
                http_only: false,
                same_site: None,
                expires: None,
            })
            .is_err()
        );
    }

    /** Listing by URL keeps domain, path and Secure rules. */
    #[test]
    fn listing_filters_by_url() {
        let cookie = CookieEntry {
            name: "a".into(),
            value: "1".into(),
            domain: ".example".into(),
            path: "/shop".into(),
            secure: true,
            http_only: false,
            same_site: "Lax".into(),
            expires: None,
        };
        assert!(sent_to(
            &cookie,
            &Url::parse("https://www.example/shop/cart").unwrap()
        ));
        assert!(!sent_to(
            &cookie,
            &Url::parse("http://www.example/shop").unwrap()
        ));
        assert!(!sent_to(
            &cookie,
            &Url::parse("https://www.example/shopping").unwrap()
        ));
        assert!(!sent_to(
            &cookie,
            &Url::parse("https://other.test/shop").unwrap()
        ));
    }
}
