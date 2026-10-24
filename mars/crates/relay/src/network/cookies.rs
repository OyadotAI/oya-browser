/** A bounded RFC 6265bis cookie jar: host and Domain cookies, SameSite, prefixes and LRU eviction. */
use crate::network::constants::{
    COOKIE_ATTRIBUTE_BYTES, COOKIE_COUNT, COOKIE_FIELD_BYTES, COOKIE_JAR_BYTES, COOKIE_LIFETIME,
    COOKIE_PAIR_BYTES, COOKIE_PROFILE_BYTES, DOMAIN_COOKIES,
};
use crate::profile::ProfileError;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use url::Url;

/// Session state belongs to one coordinator and disappears with that owner. Records keep
/// creation order, which is the RFC 6265bis tie-break when two cookies have equal paths.
#[derive(Clone, Default)]
pub struct CookieJar {
    records: Vec<Cookie>,
    /** Monotonic access counter; the smallest `access` is the least recently used cookie. */
    clock: u64,
}
/** Stored records cannot be serialized into agent responses. */
#[derive(Clone)]
struct Cookie {
    name: String,
    value: String,
    /** The exact host for a host-only cookie, otherwise the Domain attribute without a dot. */
    domain: String,
    /** Host-only cookies match one host; Domain cookies also match its subdomains. */
    host_only: bool,
    path: String,
    secure: bool,
    /** HttpOnly cookies go only on HTTP requests, never to document.cookie. */
    http_only: bool,
    same_site: SameSite,
    expires: Option<SystemTime>,
    /** Last time this cookie was set or sent, on the jar's clock, for LRU eviction. */
    access: u64,
}
/** RFC 6265bis section 5.4.7. A missing or unknown value behaves as Lax, as in Chromium. */
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum SameSite {
    Strict,
    Lax,
    None,
}
/** Which API is writing or reading: HTTP may touch HttpOnly cookies, script never can. */
#[derive(Clone, Copy, PartialEq)]
enum Api {
    Http,
    Script,
}
/// One cookie as a trusted inspection client sees it: Chromium's `Network.Cookie`, HttpOnly
/// value included. Scripts never get this view; `document_cookie` is theirs.
#[derive(Clone, Debug, Serialize)]
pub struct CookieEntry {
    pub name: String,
    pub value: String,
    /// Chromium's spelling: a leading dot marks a Domain cookie, none a host-only cookie.
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    /// `Strict`, `Lax` or `None`.
    pub same_site: String,
    /// Seconds since the Unix epoch; None for a session cookie.
    pub expires: Option<u64>,
}

impl CookieJar {
    /// Every unexpired cookie, for the inspection protocol's cookie listing.
    pub fn entries(&mut self, now: SystemTime) -> Vec<CookieEntry> {
        self.expire(now);
        self.records
            .iter()
            .map(|cookie| CookieEntry {
                name: cookie.name.clone(),
                value: cookie.value.clone(),
                domain: if cookie.host_only {
                    cookie.domain.clone()
                } else {
                    format!(".{}", cookie.domain)
                },
                path: cookie.path.clone(),
                secure: cookie.secure,
                http_only: cookie.http_only,
                same_site: match cookie.same_site {
                    SameSite::Strict => "Strict".into(),
                    SameSite::Lax => "Lax".into(),
                    SameSite::None => "None".into(),
                },
                expires: cookie.expires.and_then(|at| {
                    at.duration_since(SystemTime::UNIX_EPOCH)
                        .ok()
                        .map(|since| since.as_secs())
                }),
            })
            .collect()
    }
    /// Remove the cookies named `name` whose domain is `domain` (host-only or Domain, dot or not)
    /// and, when given, whose path is exactly `path`; returns how many went.
    pub fn remove(&mut self, name: &str, domain: &str, path: Option<&str>) -> usize {
        let domain = domain.trim_start_matches('.');
        let before = self.records.len();
        self.records.retain(|cookie| {
            !(cookie.name == name
                && cookie.domain == domain
                && path.is_none_or(|path| cookie.path == path))
        });
        before - self.records.len()
    }
    /// Forget every cookie.
    pub fn clear(&mut self) {
        self.records.clear();
    }
    /// Persisted recovery records make an uncertain profile fail closed after restart.
    pub fn recovery_record() -> Vec<u8> {
        br#"{"version":255,"cookies":[]}"#.to_vec()
    }
    /// Empty profiles can be cleared without claiming a session was removed.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    /// The Cookie request header for an HTTP request, HttpOnly cookies included. `initiator` is
    /// the site that caused the request; None means the person (typed URL, bookmark), which is
    /// same-site. `safe` marks a top-level navigation with a safe method, where Lax cookies go
    /// even cross-site. Sorted by longest path, then creation order.
    pub fn header(
        &mut self,
        url: &Url,
        initiator: Option<&Url>,
        safe: bool,
        now: SystemTime,
    ) -> String {
        let same = initiator.is_none_or(|site| same_site(url, site));
        self.select(url, same, safe, Api::Http, now)
    }
    /// The `document.cookie` getter for a document at `url`: the same cookies its own
    /// navigation would carry, minus HttpOnly ones, which scripts must never read.
    pub fn document_cookie(&mut self, url: &Url, now: SystemTime) -> String {
        self.select(url, true, true, Api::Script, now)
    }
    /// The `document.cookie` setter. Refuses HttpOnly cookies and any write that would replace
    /// an existing HttpOnly cookie, so script cannot forge or clobber a server session.
    pub fn set_document_cookie(
        &mut self,
        url: &Url,
        field: &str,
        now: SystemTime,
    ) -> Result<(), &'static str> {
        self.expire(now);
        self.insert(url, field, Api::Script, now)
    }
    /// Return value-free diagnostics; malformed and unauthorized cookies never gain authority.
    pub fn receive(&mut self, url: &Url, fields: &[String], now: SystemTime) -> Vec<String> {
        self.expire(now);
        let mut diagnostics = Vec::new();
        for field in fields {
            if let Err(reason) = self.insert(url, field, Api::Http, now)
                && diagnostics.len() < crate::constants::DIAGNOSTICS
                && !diagnostics.iter().any(|s| s == reason)
            {
                diagnostics.push(reason.into());
            }
        }
        diagnostics
    }
    /// Encode a complete bounded jar for atomic Keychain replacement, in the current version.
    pub fn profile_record(&self) -> Result<Vec<u8>, ProfileError> {
        let record = StoredJar {
            version: PROFILE_VERSION,
            cookies: self.records.iter().map(StoredCookie::from).collect(),
        };
        let bytes = serde_json::to_vec(&record).map_err(|_| ProfileError::Corrupt)?;
        validate_size(&bytes)?;
        Ok(bytes)
    }
    /// Restore the whole record or reject it, never accepting a partially valid jar. Version 1
    /// records (host-only, no HttpOnly flag) still load; they are rewritten as version 2.
    pub fn from_profile(bytes: &[u8], now: SystemTime) -> Result<Self, ProfileError> {
        validate_size(bytes)?;
        let stored: StoredJar = serde_json::from_slice(bytes).map_err(|_| ProfileError::Corrupt)?;
        if !(1..=PROFILE_VERSION).contains(&stored.version) || stored.cookies.len() > COOKIE_COUNT {
            return Err(ProfileError::Corrupt);
        }
        let mut jar = Self::default();
        let mut identities = std::collections::HashSet::new();
        for item in stored.cookies {
            let cookie = item.restore(stored.version, now)?;
            if !identities.insert(cookie.identity()) {
                return Err(ProfileError::Corrupt);
            }
            jar.clock = jar.clock.max(cookie.access);
            if cookie.expires.is_none_or(|expiry| expiry > now) {
                jar.records.push(cookie);
            }
        }
        jar.validate_capacity()?;
        Ok(jar)
    }
    /** Applicable cookies for one request, marking each as just used. */
    fn select(&mut self, url: &Url, same: bool, safe: bool, api: Api, now: SystemTime) -> String {
        self.expire(now);
        self.clock += 1;
        let clock = self.clock;
        let mut records = self
            .records
            .iter_mut()
            .filter(|c| c.applies(url, same, safe, api))
            .map(|c| {
                c.access = clock;
                &*c
            })
            .collect::<Vec<_>>();
        records.sort_by_key(|c| std::cmp::Reverse(c.path.len()));
        records
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ")
    }
    /** Expiry happens before both matching and capacity decisions. */
    fn expire(&mut self, now: SystemTime) {
        self.records
            .retain(|c| c.expires.is_none_or(|expiry| expiry > now));
    }
    /// Persisted records retain the same per-domain, aggregate and byte budgets.
    fn validate_capacity(&self) -> Result<(), ProfileError> {
        let mut sites = std::collections::HashMap::<&str, usize>::new();
        for cookie in &self.records {
            *sites.entry(site(&cookie.domain)).or_default() += 1;
        }
        if self.bytes() > COOKIE_JAR_BYTES || sites.values().any(|count| *count > DOMAIN_COOKIES) {
            return Err(ProfileError::Corrupt);
        }
        Ok(())
    }
    /** Stored bytes the jar budget counts. */
    fn bytes(&self) -> usize {
        self.records.iter().map(Cookie::bytes).sum()
    }
    /** Replacement keeps creation order; capacity is restored by evicting least recently used. */
    fn insert(
        &mut self,
        url: &Url,
        field: &str,
        api: Api,
        now: SystemTime,
    ) -> Result<(), &'static str> {
        let mut cookie = parse(url, field, api, now)?;
        if url.scheme() != "https" && self.shadows_secure(&cookie) {
            return Err("An insecure response cannot replace a Secure cookie.");
        }
        let identity = cookie.identity();
        let index = self.records.iter().position(|c| c.identity() == identity);
        if api == Api::Script && index.is_some_and(|i| self.records[i].http_only) {
            return Err("Script cannot replace an HttpOnly cookie.");
        }
        if cookie.expires.is_some_and(|expiry| expiry <= now) {
            if let Some(index) = index {
                self.records.remove(index);
            }
            return Ok(());
        }
        self.clock += 1;
        cookie.access = self.clock;
        let domain = cookie.domain.clone();
        match index {
            Some(index) => self.records[index] = cookie,
            None => self.records.push(cookie),
        }
        self.evict(site(&domain));
        Ok(())
    }
    /** RFC 6265bis section 5.7 step 15: same name, overlapping domain and path, and Secure. */
    fn shadows_secure(&self, cookie: &Cookie) -> bool {
        self.records.iter().any(|c| {
            c.secure
                && c.name == cookie.name
                && (domain_matches(&cookie.domain, &c.domain)
                    || domain_matches(&c.domain, &cookie.domain))
                && path_matches(&cookie.path, &c.path)
        })
    }
    /** Evicts least recently used cookies: first within the site, then across the jar. */
    fn evict(&mut self, site_name: &str) {
        let site_name = site_name.to_owned();
        while self
            .records
            .iter()
            .filter(|c| site(&c.domain) == site_name)
            .count()
            > DOMAIN_COOKIES
        {
            self.remove_oldest(|c| site(&c.domain) == site_name);
        }
        while self.records.len() > COOKIE_COUNT || self.bytes() > COOKIE_JAR_BYTES {
            self.remove_oldest(|_| true);
        }
    }
    /** Removes the least recently used cookie among those `scope` accepts. */
    fn remove_oldest(&mut self, scope: impl Fn(&Cookie) -> bool) {
        let oldest = self
            .records
            .iter()
            .enumerate()
            .filter(|(_, c)| scope(c))
            .min_by_key(|(_, c)| c.access)
            .map(|(index, _)| index);
        if let Some(index) = oldest {
            self.records.remove(index);
        }
    }
}

impl Cookie {
    /** Chromium's identity: a host cookie and a Domain cookie for the same name never collide. */
    fn identity(&self) -> (String, String, bool, String) {
        (
            self.name.clone(),
            self.domain.clone(),
            self.host_only,
            self.path.clone(),
        )
    }
    /** Stored bytes counted against the jar budget. */
    fn bytes(&self) -> usize {
        self.name.len() + self.value.len() + self.domain.len() + self.path.len()
    }
    /** RFC 6265bis section 5.8.3 retrieval: domain, path, Secure, HttpOnly and SameSite. */
    fn applies(&self, url: &Url, same: bool, safe: bool, api: Api) -> bool {
        let host = url.host_str().unwrap_or_default();
        let domain = if self.host_only {
            host == self.domain
        } else {
            domain_matches(host, &self.domain)
        };
        domain
            && path_matches(url.path(), &self.path)
            && (!self.secure || url.scheme() == "https")
            && (api == Api::Http || !self.http_only)
            && (same
                || self.same_site == SameSite::None
                || (safe && self.same_site == SameSite::Lax))
    }
}

/// Version 2 adds Domain cookies, HttpOnly and LRU order; version 1 records still load.
const PROFILE_VERSION: u8 = 2;

/** The jar record has its own budget, sized for a realistic 3000-cookie jar. */
fn validate_size(bytes: &[u8]) -> Result<(), ProfileError> {
    if bytes.len() > COOKIE_PROFILE_BYTES {
        return Err(ProfileError::Corrupt);
    }
    Ok(())
}

/// The complete JSON record is private to the Keychain adapter.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredJar {
    version: u8,
    cookies: Vec<StoredCookie>,
}

/// Expiry uses Unix seconds because HTTP cookie dates have second precision. Version 1 has only
/// the fields without defaults; there `host` was always an exact host.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredCookie {
    name: String,
    value: String,
    host: String,
    path: String,
    secure: bool,
    same_site: SameSite,
    expires: Option<u64>,
    #[serde(default = "host_only_default")]
    host_only: bool,
    #[serde(default)]
    http_only: bool,
    #[serde(default)]
    access: u64,
}

/** Version 1 stored only host-only cookies. */
fn host_only_default() -> bool {
    true
}

impl From<&Cookie> for StoredCookie {
    fn from(cookie: &Cookie) -> Self {
        Self {
            name: cookie.name.clone(),
            value: cookie.value.clone(),
            host: cookie.domain.clone(),
            path: cookie.path.clone(),
            secure: cookie.secure,
            same_site: cookie.same_site.clone(),
            expires: cookie
                .expires
                .and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs()),
            host_only: cookie.host_only,
            http_only: cookie.http_only,
            access: cookie.access,
        }
    }
}

impl StoredCookie {
    /// Reparse through response-cookie policy before trusting persisted fields.
    fn restore(self, version: u8, now: SystemTime) -> Result<Cookie, ProfileError> {
        if version == 1 && (!self.host_only || self.http_only || self.access != 0) {
            return Err(ProfileError::Corrupt);
        }
        let scheme = if self.secure { "https" } else { "http" };
        let url =
            Url::parse(&format!("{scheme}://{}/", self.host)).map_err(|_| ProfileError::Corrupt)?;
        if url.host_str() != Some(self.host.as_str()) {
            return Err(ProfileError::Corrupt);
        }
        let mut cookie =
            parse(&url, &self.field(), Api::Http, now).map_err(|_| ProfileError::Corrupt)?;
        if cookie.name != self.name
            || cookie.value != self.value
            || cookie.domain != self.host
            || cookie.host_only != self.host_only
            || cookie.path != self.path
            || cookie.secure != self.secure
            || cookie.same_site != self.same_site
        {
            return Err(ProfileError::Corrupt);
        }
        cookie.access = self.access;
        cookie.expires = self
            .expires
            .and_then(|seconds| SystemTime::UNIX_EPOCH.checked_add(Duration::from_secs(seconds)));
        if self.expires.is_some() && cookie.expires.is_none() {
            return Err(ProfileError::Corrupt);
        }
        Ok(cookie)
    }
    /** The Set-Cookie field that would recreate this record. */
    fn field(&self) -> String {
        let same_site = match self.same_site {
            SameSite::Strict => "Strict",
            SameSite::Lax => "Lax",
            SameSite::None => "None",
        };
        let secure = if self.secure { "; Secure" } else { "" };
        let http_only = if self.http_only { "; HttpOnly" } else { "" };
        let domain = if self.host_only {
            String::new()
        } else {
            format!("; Domain={}", self.host)
        };
        format!(
            "{}={}; Path={}{secure}{http_only}{domain}; SameSite={same_site}",
            self.name, self.value, self.path
        )
    }
}
/** Invalid records are rejected whole; secrets are never included in the reason. */
fn parse(url: &Url, field: &str, api: Api, now: SystemTime) -> Result<Cookie, &'static str> {
    if field.len() > COOKIE_FIELD_BYTES
        || !field.is_ascii()
        || field.chars().any(|c| c.is_control() && c != '\t')
    {
        return Err("A response cookie is invalid or too large.");
    }
    let mut parts = field.split(';');
    let (name, value) = parts
        .next()
        .and_then(|p| p.split_once('='))
        .map(|(name, value)| (name.trim(), value.trim()))
        .ok_or("A response cookie is invalid.")?;
    if !valid_pair(name, value) {
        return Err("A response cookie is invalid or too large.");
    }
    let host = url.host_str().unwrap_or_default();
    let mut cookie = Cookie {
        name: name.into(),
        value: value.into(),
        domain: host.into(),
        host_only: true,
        path: default_path(url.path()),
        secure: false,
        http_only: false,
        same_site: SameSite::Lax,
        expires: None,
        access: 0,
    };
    let attributes = attributes(parts, &mut cookie, now);
    if let Some(domain) = attributes.domain {
        apply_domain(&mut cookie, host, &domain)?;
    }
    if !attributes.explicit_path && name.to_ascii_lowercase().starts_with("__host-") {
        return Err("A response cookie violates its security requirements.");
    }
    check_security(url, &cookie, api)?;
    Ok(cookie)
}

/** Names are tokens; values are printable ASCII without `;` (browsers accept spaces and quotes). */
fn valid_pair(name: &str, value: &str) -> bool {
    !name.is_empty()
        && name.len() + value.len() <= COOKIE_PAIR_BYTES
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
        && value
            .bytes()
            .all(|b| (0x20..0x7f).contains(&b) && b != b';')
}

/** Attribute values the parser carries out of the attribute loop. */
#[derive(Default)]
struct Attributes {
    /** Lowercased Domain value without a leading dot, when one was given. */
    domain: Option<String>,
    /** Whether a valid Path attribute was given; `__Host-` requires one. */
    explicit_path: bool,
}

/** RFC 6265bis section 5.6: the last occurrence of an attribute wins; Max-Age beats Expires. */
fn attributes<'a>(
    parts: impl Iterator<Item = &'a str>,
    cookie: &mut Cookie,
    now: SystemTime,
) -> Attributes {
    let mut found = Attributes::default();
    let mut age = None;
    let mut expires = None;
    for part in parts {
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        let value = value.trim();
        if value.len() > COOKIE_ATTRIBUTE_BYTES {
            continue;
        }
        match key.trim().to_ascii_lowercase().as_str() {
            "domain" if !value.is_empty() => {
                found.domain = Some(value.trim_start_matches('.').to_ascii_lowercase());
            }
            "secure" => cookie.secure = true,
            "httponly" => cookie.http_only = true,
            "path" if value.starts_with('/') => {
                cookie.path = value.into();
                found.explicit_path = true;
            }
            "max-age" => age = max_age(value).or(age),
            "expires" => expires = cookie_date(value).or(expires),
            "samesite" => cookie.same_site = same_site_value(value),
            _ => {}
        }
    }
    cookie.expires = age
        .map(|age| expiry_after(now, age))
        .or(expires)
        .map(|time| time.min(now + COOKIE_LIFETIME));
    found
}

/** Unknown values behave as Lax, matching Chromium's default for unspecified SameSite. */
fn same_site_value(value: &str) -> SameSite {
    if value.eq_ignore_ascii_case("strict") {
        SameSite::Strict
    } else if value.eq_ignore_ascii_case("none") {
        SameSite::None
    } else {
        SameSite::Lax
    }
}

/** RFC 6265bis section 5.6.2: an optional `-` then digits; anything else is ignored. */
fn max_age(value: &str) -> Option<i64> {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(value.parse::<i64>().unwrap_or(if value.starts_with('-') {
        i64::MIN
    } else {
        i64::MAX
    }))
}

/** Zero or negative Max-Age expires immediately; larger ages are clamped by the caller. */
fn expiry_after(now: SystemTime, age: i64) -> SystemTime {
    if age <= 0 {
        return SystemTime::UNIX_EPOCH;
    }
    let age = Duration::from_secs(age.unsigned_abs()).min(COOKIE_LIFETIME);
    now + age
}

/**
 * RFC 6265bis section 5.7 steps 5 to 8: a Domain must cover the setting host and must not be a
 * public suffix. Setting a public suffix that is the host itself degrades to host-only; an IP
 * host only accepts its own address. Otherwise the cookie becomes a Domain cookie.
 */
fn apply_domain(cookie: &mut Cookie, host: &str, domain: &str) -> Result<(), &'static str> {
    if cookie.name.to_ascii_lowercase().starts_with("__host-") {
        return Err("A response cookie violates its security requirements.");
    }
    if domain == host && (is_ip(host) || is_public_suffix(domain)) {
        return Ok(());
    }
    if is_public_suffix(domain) || !domain_matches(host, domain) {
        return Err("A response cookie names a Domain that does not cover its host.");
    }
    cookie.domain = domain.into();
    cookie.host_only = false;
    Ok(())
}

/** Secure-origin, SameSite=None, prefix and HttpOnly-from-script rules. */
fn check_security(url: &Url, cookie: &Cookie, api: Api) -> Result<(), &'static str> {
    let name = cookie.name.to_ascii_lowercase();
    let needs_secure = cookie.same_site == SameSite::None
        || name.starts_with("__secure-")
        || name.starts_with("__host-");
    if (cookie.secure && url.scheme() != "https")
        || (needs_secure && !cookie.secure)
        || (name.starts_with("__host-") && (!cookie.host_only || cookie.path != "/"))
    {
        return Err("A response cookie violates its security requirements.");
    }
    if api == Api::Script && cookie.http_only {
        return Err("Script cannot set an HttpOnly cookie.");
    }
    Ok(())
}

/** RFC 6265bis section 5.1.3: identical, or a dot-separated suffix of a non-IP host. */
fn domain_matches(host: &str, domain: &str) -> bool {
    host == domain
        || (!is_ip(host)
            && host.len() > domain.len()
            && host.ends_with(domain)
            && host.as_bytes()[host.len() - domain.len() - 1] == b'.')
}

/** IPv4 literals and bracketed IPv6 literals never domain-match anything but themselves. */
fn is_ip(host: &str) -> bool {
    host.starts_with('[') || host.parse::<std::net::Ipv4Addr>().is_ok()
}

/** A suffix in the Public Suffix List, or an unlisted single label, which its `*` rule covers. */
fn is_public_suffix(domain: &str) -> bool {
    psl::suffix_str(domain) == Some(domain)
}

/// The registrable domain (eTLD+1), or the host itself for IPs and bare suffixes. Cookie
/// limits, SameSite and Sec-Fetch-Site all compare sites this way.
pub(super) fn site(host: &str) -> &str {
    if is_ip(host) {
        return host;
    }
    psl::domain_str(host).unwrap_or(host)
}

/** Schemeful same-site: same scheme and same registrable domain (RFC 6265bis section 5.2). */
fn same_site(url: &Url, initiator: &Url) -> bool {
    url.scheme() == initiator.scheme()
        && site(url.host_str().unwrap_or_default())
            == site(initiator.host_str().unwrap_or_default())
}

/** Slash boundaries prevent /account from matching /accounting. */
fn path_matches(request: &str, cookie: &str) -> bool {
    request == cookie
        || (request.starts_with(cookie)
            && (cookie.ends_with('/') || request.as_bytes().get(cookie.len()) == Some(&b'/')))
}
/** RFC default paths exclude the final path segment. */
fn default_path(path: &str) -> String {
    path.rfind('/')
        .filter(|i| *i > 0)
        .map(|i| path[..i].into())
        .unwrap_or_else(|| "/".into())
}

/** Month names RFC 6265bis section 5.1.1 recognizes by their first three letters. */
const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/**
 * RFC 6265bis section 5.1.1 cookie-date parsing, which accepts the formats real servers send
 * (`Thu, 01-Jan-2026 00:00:00 GMT`, two-digit years) that strict HTTP-date parsing refuses.
 */
fn cookie_date(value: &str) -> Option<SystemTime> {
    let (mut time, mut day, mut month, mut year) = (None, None, None, None);
    for token in value
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == ':'))
        .filter(|token| !token.is_empty())
    {
        if time.is_none()
            && let Some(found) = clock(token)
        {
            time = Some(found);
        } else if day.is_none()
            && let Some(found) = leading_number(token, 1, 2)
        {
            day = Some(found);
        } else if month.is_none()
            && let Some(found) = month_of(token)
        {
            month = Some(found);
        } else if year.is_none()
            && let Some(found) = leading_number(token, 2, 4)
        {
            year = Some(found);
        }
    }
    let (hour, minute, second) = time?;
    let (day, month, mut year) = (day?, month?, year?);
    year += match year {
        70..=99 => 1900,
        0..=69 => 2000,
        _ => 0,
    };
    if !(1..=31).contains(&day) || year < 1601 || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let days = days_from_civil(i64::from(year), month, day);
    let seconds = days * 86_400 + i64::from(hour * 3600 + minute * 60 + second);
    Some(match u64::try_from(seconds) {
        Ok(seconds) => SystemTime::UNIX_EPOCH + Duration::from_secs(seconds),
        Err(_) => SystemTime::UNIX_EPOCH,
    })
}

/** `hh:mm:ss`: hours and minutes are one or two digits, seconds may be followed by non-digits. */
fn clock(token: &str) -> Option<(u32, u32, u32)> {
    let mut fields = token.splitn(3, ':');
    let hour = short_number(fields.next()?)?;
    let minute = short_number(fields.next()?)?;
    let second = leading_number(fields.next()?, 1, 2)?;
    Some((hour, minute, second))
}

/** Exactly one or two digits. */
fn short_number(field: &str) -> Option<u32> {
    (1..=2).contains(&field.len()).then_some(())?;
    leading_number(field, field.len(), field.len())
}

/** Between `min` and `max` leading digits, followed by nothing or a non-digit. */
fn leading_number(token: &str, min: usize, max: usize) -> Option<u32> {
    let digits = token.bytes().take_while(u8::is_ascii_digit).count();
    if digits < min || digits > max {
        return None;
    }
    token[..digits].parse().ok()
}

/** A month token: its first three letters name a month. */
fn month_of(token: &str) -> Option<u32> {
    let prefix = token.get(..3)?.to_ascii_lowercase();
    MONTHS
        .iter()
        .position(|m| *m == prefix)
        .map(|i| i as u32 + 1)
}

/** Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm). */
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year =
        (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests;
