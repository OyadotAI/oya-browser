/** The person's browser settings: appearance, home page, downloads folder and the bookmarks bar. */
use crate::profile::{ProfileError, session_url_valid};
use serde::{Deserialize, Serialize};

/// Version rejects saved settings whose meaning this build does not know.
pub const SETTINGS_VERSION: u32 = 1;
/// Settings are a handful of fields; anything bigger is not ours.
pub const SETTINGS_BYTES: usize = 8 * 1024;
/// A downloads folder path longer than this is not one the person picked.
const PATH_BYTES: usize = 1024;

/** The window's appearance; System follows macOS. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    /// Follow the macOS setting.
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

/** Where the address field sends a query that is not an address. DuckDuckGo by default. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchEngine {
    /// duckduckgo.com, the default.
    #[default]
    DuckDuckGo,
    /// google.com.
    Google,
    /// bing.com.
    Bing,
}

impl SearchEngine {
    /** The engine's results address for one query. */
    pub fn search_url(self, query: &str) -> String {
        let base = match self {
            Self::DuckDuckGo => "https://duckduckgo.com/?q=",
            Self::Google => "https://www.google.com/search?q=",
            Self::Bing => "https://www.bing.com/search?q=",
        };
        let encoded: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
        format!("{base}{encoded}")
    }
}

/** Everything the Settings window changes, saved as one record. */
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Format version, checked on every read.
    pub version: u32,
    /// The window's appearance.
    pub appearance: Appearance,
    /// Where the home button goes; none means the built-in start page.
    pub home_page: Option<String>,
    /// Where downloads are saved; none means ~/Downloads.
    pub downloads_folder: Option<String>,
    /// Whether the bookmarks bar shows under the toolbar (Cmd+Shift+B).
    pub bookmarks_bar: bool,
    /// Where address-field searches go.
    #[serde(default)]
    pub search_engine: SearchEngine,
}

impl Default for Settings {
    /** Defaults match a fresh Oya profile: system appearance, start page, ~/Downloads, bar hidden. */
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            appearance: Appearance::System,
            home_page: None,
            downloads_folder: None,
            bookmarks_bar: false,
            search_engine: SearchEngine::DuckDuckGo,
        }
    }
}

impl Settings {
    /** A home page must be an address Mars may keep; anything else is refused, not stored. */
    pub fn set_home_page(&mut self, value: &str) -> bool {
        let value = value.trim();
        if value.is_empty() {
            self.home_page = None;
            return true;
        }
        let ok = session_url_valid(value);
        if ok {
            self.home_page = Some(value.into());
        }
        ok
    }

    /** Serialize after checking every bound a read would check. */
    pub fn encode(&self) -> Result<Vec<u8>, ProfileError> {
        let valid = self.version == SETTINGS_VERSION
            && self.home_page.as_deref().is_none_or(session_url_valid)
            && self.downloads_folder.as_deref().is_none_or(folder_valid);
        if !valid {
            return Err(ProfileError::Corrupt);
        }
        let bytes = serde_json::to_vec(self).map_err(|_| ProfileError::Corrupt)?;
        if bytes.len() > SETTINGS_BYTES {
            return Err(ProfileError::Corrupt);
        }
        Ok(bytes)
    }

    /** Read a saved record as untrusted input. */
    pub fn decode(bytes: &[u8]) -> Result<Self, ProfileError> {
        if bytes.len() > SETTINGS_BYTES {
            return Err(ProfileError::Corrupt);
        }
        let settings: Self = serde_json::from_slice(bytes).map_err(|_| ProfileError::Corrupt)?;
        settings.encode()?;
        Ok(settings)
    }
}

/** A downloads folder is an absolute path without NULs, of a sane length. */
pub fn folder_valid(path: &str) -> bool {
    path.len() <= PATH_BYTES && path.starts_with('/') && !path.contains('\0')
}

/** What the person's address-field text means, as Chrome reads it: a full address stays as typed; a host without a scheme gets https:// (http:// for localhost and IP addresses); anything else, including text with spaces, is a search. Only the human address field uses this; agent navigation still needs a full URL. */
pub fn address_input(text: &str, engine: SearchEngine) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text.contains("://") {
        return Some(text.into());
    }
    if text.chars().any(char::is_whitespace) {
        return Some(engine.search_url(text));
    }
    let host_port = text.split(['/', '?', '#']).next().unwrap_or_default();
    if local_host(host_port) {
        return Some(format!("http://{text}"));
    }
    if domain_like(host_port) {
        return Some(format!("https://{text}"));
    }
    Some(engine.search_url(text))
}

/** localhost, an IPv4 address or a bracketed IPv6 address, each with an optional port. */
fn local_host(host_port: &str) -> bool {
    let host = if host_port.starts_with('[') {
        return host_port.find(']').is_some_and(|end| {
            host_port[1..end].parse::<std::net::Ipv6Addr>().is_ok()
                && port_ok(&host_port[end + 1..])
        });
    } else {
        match host_port.split_once(':') {
            Some((host, port)) if port_ok(&format!(":{port}")) => host,
            Some(_) => return false,
            None => host_port,
        }
    };
    host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::Ipv4Addr>().is_ok()
}

/** An optional ":port" suffix of digits. */
fn port_ok(rest: &str) -> bool {
    rest.is_empty()
        || rest
            .strip_prefix(':')
            .is_some_and(|port| port.parse::<u16>().is_ok())
}

/** A dotted host name with a letter top-level label, optionally with a port: example.com, docs.rs:443. */
fn domain_like(host_port: &str) -> bool {
    let (host, port) = match host_port.split_once(':') {
        Some((host, port)) => (host, format!(":{port}")),
        None => (host_port, String::new()),
    };
    let labels: Vec<&str> = host.split('.').collect();
    port_ok(&port)
        && labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && label.chars().all(|c| c.is_alphanumeric() || c == '-')
        })
        && labels
            .last()
            .is_some_and(|tld| tld.chars().count() >= 2 && tld.chars().all(char::is_alphabetic))
}
