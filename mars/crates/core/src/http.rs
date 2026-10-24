/** Shared HTTP method data does not require a networking implementation. */
use serde::{Deserialize, Serialize};

/// The methods Mars sends. Navigations and forms use only GET and POST; page scripts (fetch and
/// XHR) may use all of them. Every method but GET and HEAD is unsafe: admitted like a form POST,
/// never retried, cached or replayed.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum Method {
    /// Safe navigation is the default for existing callers.
    #[default]
    Get,
    /// An authorized form transaction may have external effects.
    Post,
    /// A GET without a response body; safe, but its empty answer is never cached for the GET.
    Head,
    /// A script replacing a resource.
    Put,
    /// A script changing part of a resource.
    Patch,
    /// A script removing a resource.
    Delete,
    /// A script asking what a resource allows; a CORS preflight is not this, it rides on GET.
    Options,
}

impl Method {
    /// The method as a page script names it, normalized the Fetch way: GET, HEAD, POST, PUT,
    /// DELETE and OPTIONS match in any case, PATCH only in upper case. Anything else (including
    /// the forbidden CONNECT, TRACE and TRACK) is None, so it never reaches the network.
    pub fn parse(name: &str) -> Option<Self> {
        let method = match name.to_ascii_uppercase().as_str() {
            "GET" => Self::Get,
            "POST" => Self::Post,
            "HEAD" => Self::Head,
            "PUT" => Self::Put,
            "DELETE" => Self::Delete,
            "OPTIONS" => Self::Options,
            _ if name == "PATCH" => Self::Patch,
            _ => return None,
        };
        Some(method)
    }

    /// The method's name on the wire and in CORS comparisons.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Head => "HEAD",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
        }
    }

    /// GET and HEAD have no external effects. Every other method gets POST's protections: effect
    /// admission before it is sent, no automatic retry, no caching, and no history replay.
    pub fn is_safe(&self) -> bool {
        matches!(self, Self::Get | Self::Head)
    }
}

/// Canonical name of the UTF-8 encoding, shared by Relay's decoder and Eye's meta check.
pub const UTF_8: &str = "utf-8";
/// Canonical name of windows-1252, which WHATWG also uses for Latin-1 and ASCII labels.
pub const WINDOWS_1252: &str = "windows-1252";

/** WHATWG labels for UTF-8; any other spelling is not UTF-8. */
const UTF8_LABELS: [&str; 6] = [
    "unicode-1-1-utf-8",
    "unicode11utf8",
    "unicode20utf8",
    "utf-8",
    "utf8",
    "x-unicode20utf8",
];

/** WHATWG labels that all mean windows-1252, including Latin-1 and ASCII. */
const WINDOWS_1252_LABELS: [&str; 17] = [
    "ansi_x3.4-1968",
    "ascii",
    "cp1252",
    "cp819",
    "csisolatin1",
    "ibm819",
    "iso-8859-1",
    "iso-ir-100",
    "iso8859-1",
    "iso88591",
    "iso_8859-1",
    "iso_8859-1:1987",
    "l1",
    "latin1",
    "us-ascii",
    "windows-1252",
    "x-cp1252",
];

/// Resolves a charset label to the one encoding Mars decodes it as, so the transport and a meta
/// declaration compare by encoding rather than by spelling. Unsupported labels return None.
pub fn encoding_for_label(label: &str) -> Option<&'static str> {
    let label = label
        .trim_matches(|c: char| c.is_ascii_whitespace())
        .to_ascii_lowercase();
    if UTF8_LABELS.contains(&label.as_str()) {
        Some(UTF_8)
    } else if WINDOWS_1252_LABELS.contains(&label.as_str()) {
        Some(WINDOWS_1252)
    } else {
        None
    }
}
