/** The synthetic demo consumes bounded form requests without storing or logging credentials. */
use crate::constants::{FORM_BYTES, HEADER_BYTES};
use std::{
    io::Read,
    net::TcpStream,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/** All data is local to one bounded demo exchange. */
pub(super) struct Request {
    pub method: String,
    pub target: String,
    pub headers: String,
    pub body: String,
}
/** Deadline and cancellation apply to both headers and a promised POST body. */
pub(super) fn read(
    socket: &mut TcpStream,
    stop: &AtomicBool,
    deadline: Instant,
) -> Option<Request> {
    let mut data = Vec::new();
    let mut bytes = [0; 1024];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if stop.load(Ordering::Relaxed) || remaining.is_zero() {
            return None;
        }
        if let Some(end) = data.windows(4).position(|p| p == b"\r\n\r\n") {
            if end + 4 > HEADER_BYTES {
                return None;
            }
            let header = std::str::from_utf8(&data[..end]).ok()?;
            let length = header
                .lines()
                .find_map(|line| {
                    line.split_once(':')
                        .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                        .and_then(|(_, v)| v.trim().parse::<usize>().ok())
                })
                .unwrap_or_default();
            if length > FORM_BYTES {
                return None;
            }
            if data.len() >= end + 4 + length {
                let mut first = header.lines().next()?.split_whitespace();
                return Some(Request {
                    method: first.next()?.into(),
                    target: first.next()?.into(),
                    headers: header.into(),
                    body: String::from_utf8(data[end + 4..end + 4 + length].to_vec()).ok()?,
                });
            }
        } else if data.len() >= HEADER_BYTES {
            return None;
        }
        socket
            .set_read_timeout(Some(remaining.min(Duration::from_millis(250))))
            .ok()?;
        let count = socket.read(&mut bytes).ok()?;
        if count == 0 {
            return None;
        }
        data.extend_from_slice(&bytes[..count]);
    }
}
/** Responses never reflect protected fields; cookies are synthetic and session-only. */
pub(super) fn response(request: &Request) -> String {
    let path = request.target.split('?').next().unwrap_or_default();
    #[cfg(feature = "test-support")]
    if path == "/__test/clicks" {
        return reply(
            "200 OK",
            "text/html",
            include_str!("../../../fixtures/clicks.html"),
        );
    }
    #[cfg(feature = "test-support")]
    if path == "/__test/v9-error" {
        return reply(
            "200 OK",
            "text/html",
            "<p>Uncommitted script-error candidate</p><script>\nlet n=;\n</script>",
        );
    }
    if request.method == "POST" && path == "/login" {
        let fields = url::form_urlencoded::parse(request.body.as_bytes()).collect::<Vec<_>>();
        if fields.iter().any(|(k, v)| k == "username" && !v.is_empty())
            && fields.iter().any(|(k, v)| k == "password" && !v.is_empty())
        {
            return "HTTP/1.1 303 See Other\r\nLocation: /account\r\nSet-Cookie: oya_demo=session; Path=/; HttpOnly; SameSite=Lax\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into();
        }
        return reply(
            "422 Unprocessable Content",
            "text/html",
            "<h1>Complete both required fields.</h1><a href=/forms>Return to the form</a>",
        );
    }
    if path == "/account" {
        let signed_in = request.headers.lines().any(|line| {
            line.split_once(':').is_some_and(|(key, value)| {
                key.eq_ignore_ascii_case("cookie")
                    && value.split(';').any(|p| p.trim() == "oya_demo=session")
            })
        });
        return reply(
            "200 OK",
            "text/html",
            if signed_in {
                "<h1>Your synthetic session is active.</h1><p>The server received the session cookie.</p><a href=/account>Check session again</a><a href=/forms>Back to forms</a>"
            } else {
                "<h1>No demo session.</h1><a href=/forms>Open the form</a>"
            },
        );
    }
    let (mime, source) = match path {
        "/start" | "/" => ("text/html", include_str!("../../../fixtures/start.html")),
        "/demo" => ("text/html", include_str!("../../../fixtures/desktop.html")),
        "/compat/choose-option" => (
            "text/html",
            include_str!("../../../fixtures/choose-option.html"),
        ),
        "/compat/canvas" => (
            "text/html; charset=utf-8",
            include_str!("../../../fixtures/canvas.html"),
        ),
        "/forms" => ("text/html", include_str!("../../../fixtures/forms.html")),
        "/forms.css" => ("text/css", include_str!("../../../fixtures/forms.css")),
        "/reading/styled" => (
            "text/html",
            include_str!("../../../fixtures/reading/styled.html"),
        ),
        "/reading/long" => (
            "text/html",
            include_str!("../../../fixtures/reading/httpbin.html"),
        ),
        "/second" => ("text/html", include_str!("../../../fixtures/second.html")),
        _ => ("text/html", include_str!("../../../fixtures/desktop.html")),
    };
    reply("200 OK", mime, source)
}
/** Exact lengths exercise Oya's owned response framing. */
fn reply(status: &str, mime: &str, source: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {mime}; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{source}",
        source.len()
    )
}
