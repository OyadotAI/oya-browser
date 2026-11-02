/** Literal loopback URLs avoid DNS, rebinding, proxy and redirect ambiguity. */
use crate::constants::URL_BYTES;
use crate::protocol::{EngineError, EngineResult};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

/// A validated socket address and origin-form HTTP target.
pub struct Endpoint {
    /// Already resolved literal address; never passed through DNS.
    pub address: SocketAddr,
    /** Exact validated Host header value. */
    host: String,
    /** ASCII origin-form target, with controls and whitespace rejected. */
    path: String,
}

impl Endpoint {
    /// Only explicit loopback addresses and nonzero numeric ports are permitted.
    pub fn parse(url: &str) -> EngineResult<Self> {
        if url.len() > URL_BYTES {
            return Err(EngineError::limit("URL bytes", URL_BYTES));
        }
        if !url.is_ascii()
            || url.chars().any(|c| c.is_control() || c.is_whitespace())
            || url.contains(['#', '\\'])
        {
            return Err(denied());
        }
        let value = url.strip_prefix("http://").ok_or_else(denied)?;
        let (host, path) = value.split_once('/').ok_or_else(denied)?;
        let (address, port) = authority(host)?;
        Ok(Self {
            address: SocketAddr::new(address, port),
            host: host.into(),
            path: format!("/{path}"),
        })
    }

    /// Uses only validated fields; no ambient proxy or credential configuration exists.
    pub fn request(&self) -> String {
        format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: text/html\r\n\r\n",
            self.path, self.host
        )
    }
}

/** Authority parsing has exactly two allowed hosts and never normalizes an alias. */
fn authority(host: &str) -> EngineResult<(IpAddr, u16)> {
    let (address, port) = if let Some(port) = host.strip_prefix("127.0.0.1:") {
        (IpAddr::V4(Ipv4Addr::LOCALHOST), port)
    } else if let Some(port) = host.strip_prefix("[::1]:") {
        (IpAddr::V6(Ipv6Addr::LOCALHOST), port)
    } else {
        return Err(denied());
    };
    if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
        return Err(denied());
    }
    let port = port
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(denied)?;
    Ok((address, port))
}

/** The refusal is local and final; it does not trigger an approval or alternate route. */
fn denied() -> EngineError {
    EngineError::new(
        "permission_denied",
        "Only loopback HTTP URLs are supported.",
        "Use http://127.0.0.1:<port>/<path> or http://[::1]:<port>/<path>.",
    )
}
