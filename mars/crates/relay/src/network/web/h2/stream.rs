/** One HTTP/2 request stream: its request body, response headers, bounded body and outcome. */
use super::frame;
use crate::network::constants::H2_STREAM_WINDOW;
use crate::network::headers::insert_field;
use crate::protocol::EngineError;
use std::collections::BTreeMap;

/** RFC 9113 section 8.2.2: connection-specific fields have no meaning in HTTP/2; dropped. */
const CONNECTION_FIELDS: [&str; 5] = [
    "connection",
    "keep-alive",
    "proxy-connection",
    "transfer-encoding",
    "upgrade",
];

/// How a stream ended.
pub(super) enum Ending {
    /// A complete response.
    Complete,
    /// No usable response. `retry` means the server never processed the request.
    Failed { retry: bool, error: EngineError },
}

/// Request and response state for one stream.
pub(super) struct Stream {
    pub status: Option<u16>,
    pub headers: BTreeMap<String, String>,
    pub set_cookies: Vec<String>,
    pub body: Vec<u8>,
    /// Answers a HEAD: no DATA is expected and Content-Length describes the GET's body.
    pub head: bool,
    limit: usize,
    /** Request body bytes not yet sent; None once END_STREAM went out. */
    pending: Option<Vec<u8>>,
    send_window: i64,
    /** Bytes the server may still send on this stream before Relay's next WINDOW_UPDATE. */
    receive_window: i64,
    /** DATA bytes received since this stream's last WINDOW_UPDATE. */
    unacknowledged: i64,
    pub ending: Option<Ending>,
}

impl Stream {
    /// A new stream that may receive `limit` body bytes and must send `body`.
    pub(super) fn new(limit: usize, window: i64, body: Vec<u8>) -> Self {
        Self {
            status: None,
            headers: BTreeMap::new(),
            set_cookies: Vec::new(),
            body: Vec::new(),
            head: false,
            limit,
            pending: (!body.is_empty()).then_some(body),
            send_window: window,
            receive_window: i64::from(H2_STREAM_WINDOW),
            unacknowledged: 0,
            ending: None,
        }
    }

    /// Charges one DATA frame (padding included) to the receive window. Like Chrome, once more
    /// than half the window is consumed it returns the WINDOW_UPDATE increment that restores
    /// it, so bodies longer than the initial window keep flowing. An Err is the RST_STREAM
    /// code for a server that overran the window (section 6.9.1).
    pub(super) fn receive(&mut self, size: i64) -> Result<Option<u32>, u32> {
        self.receive_window -= size;
        if self.receive_window < 0 {
            return Err(frame::FLOW_CONTROL_ERROR);
        }
        self.unacknowledged += size;
        if self.ending.is_some() || self.unacknowledged <= i64::from(H2_STREAM_WINDOW) / 2 {
            return Ok(None);
        }
        let increment = std::mem::take(&mut self.unacknowledged);
        self.receive_window += increment;
        Ok(Some(increment as u32))
    }

    /// The first outcome sticks; later failures do not replace it.
    pub(super) fn fail(&mut self, retry: bool, error: EngineError) {
        if self.ending.is_none() {
            self.ending = Some(Ending::Failed { retry, error });
        }
    }

    /// Applies a SETTINGS or WINDOW_UPDATE change to the send window, refusing overflow.
    pub(super) fn grow(&mut self, delta: i64) -> Result<(), ()> {
        self.send_window += delta;
        if self.send_window > frame::MAX_WINDOW {
            return Err(());
        }
        Ok(())
    }

    /// The next request body chunk both windows allow, and whether it is the last.
    pub(super) fn next_chunk(&mut self, connection: i64, max: usize) -> Option<(Vec<u8>, bool)> {
        let pending = self.pending.as_mut()?;
        let room = connection.min(self.send_window).min(max as i64);
        if room <= 0 {
            return None;
        }
        let size = pending.len().min(room as usize);
        let chunk: Vec<u8> = pending.drain(..size).collect();
        self.send_window -= size as i64;
        let end = pending.is_empty();
        if end {
            self.pending = None;
        }
        Some((chunk, end))
    }

    /// DATA after the response headers, within the body budget. An Err is the RST_STREAM code.
    pub(super) fn data(&mut self, content: &[u8], end: bool) -> Result<(), u32> {
        if self.ending.is_some() {
            return Ok(());
        }
        if self.status.is_none() {
            return Err(frame::PROTOCOL_ERROR);
        }
        if content.len() > self.limit.saturating_sub(self.body.len()) {
            self.fail(false, EngineError::limit("HTTP body bytes", self.limit));
            return Err(frame::CANCEL);
        }
        self.body.extend_from_slice(content);
        if end {
            self.finish()?;
        }
        Ok(())
    }

    /// A decoded header block: the response head, an interim 1xx head, or trailers.
    pub(super) fn headers(&mut self, fields: Vec<(String, String)>, end: bool) -> Result<(), u32> {
        if self.ending.is_some() {
            return Ok(());
        }
        if self.status.is_some() {
            return if end {
                self.finish()
            } else {
                Err(frame::PROTOCOL_ERROR)
            };
        }
        let status = self.head(fields)?;
        if (100..200).contains(&status) {
            self.headers.clear();
            self.set_cookies.clear();
            return if end {
                Err(frame::PROTOCOL_ERROR)
            } else {
                Ok(())
            };
        }
        self.status = Some(status);
        if end {
            self.finish()?;
        }
        Ok(())
    }

    /** Section 8.3.2: `:status` first and alone among pseudo-headers; lowercase names. */
    fn head(&mut self, fields: Vec<(String, String)>) -> Result<u16, u32> {
        let mut fields = fields.into_iter();
        let (name, status) = fields.next().ok_or(frame::PROTOCOL_ERROR)?;
        if name != ":status" || status.len() != 3 || !status.bytes().all(|b| b.is_ascii_digit()) {
            return Err(frame::PROTOCOL_ERROR);
        }
        for (name, value) in fields {
            self.field(&name, &value)?;
        }
        status.parse().map_err(|_| frame::PROTOCOL_ERROR)
    }

    /** One regular field, validated like HTTP/1.1 fields plus HTTP/2's lowercase rule. */
    fn field(&mut self, name: &str, value: &str) -> Result<(), u32> {
        let valid_name = !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_".contains(&b));
        if !valid_name || value.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0) {
            return Err(frame::PROTOCOL_ERROR);
        }
        if CONNECTION_FIELDS.contains(&name) {
            return Ok(());
        }
        if name == "set-cookie" {
            self.set_cookies.push(value.trim().to_owned());
            return Ok(());
        }
        insert_field(&mut self.headers, name, value).map_err(|_| frame::PROTOCOL_ERROR)
    }

    /** END_STREAM: a declared Content-Length must equal the DATA received (section 8.1.1),
    except on a HEAD answer or a status that has no content. */
    fn finish(&mut self) -> Result<(), u32> {
        let length = self.headers.get("content-length");
        let bodyless = self.head || matches!(self.status, Some(204 | 304));
        if !bodyless
            && length.is_some_and(|length| length.parse::<usize>().ok() != Some(self.body.len()))
        {
            return Err(frame::PROTOCOL_ERROR);
        }
        self.ending = Some(Ending::Complete);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Owned fields from pairs. */
    fn fields(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(n, v)| ((*n).into(), (*v).into()))
            .collect()
    }

    #[test]
    fn a_response_completes_with_headers_cookies_and_body() {
        let mut stream = Stream::new(100, 65_535, Vec::new());
        stream
            .headers(
                fields(&[
                    (":status", "200"),
                    ("content-length", "2"),
                    ("set-cookie", "a=1"),
                    ("set-cookie", "b=2"),
                    ("connection", "close"),
                ]),
                false,
            )
            .expect("head");
        stream.data(b"ok", true).expect("body");
        assert!(matches!(stream.ending, Some(Ending::Complete)));
        assert_eq!(stream.set_cookies, ["a=1", "b=2"]);
        assert!(!stream.headers.contains_key("connection"));
    }

    /** A HEAD answer ends at its headers although Content-Length describes the GET's body. */
    #[test]
    fn a_head_answer_completes_without_its_declared_body() {
        let mut stream = Stream::new(100, 65_535, Vec::new());
        stream.head = true;
        stream
            .headers(fields(&[(":status", "200"), ("content-length", "5")]), true)
            .expect("head");
        assert!(matches!(stream.ending, Some(Ending::Complete)));
        assert!(stream.body.is_empty());
    }

    #[test]
    fn interim_responses_are_skipped() {
        let mut stream = Stream::new(100, 65_535, Vec::new());
        stream
            .headers(fields(&[(":status", "103"), ("link", "</a.css>")]), false)
            .expect("interim");
        stream
            .headers(fields(&[(":status", "204")]), true)
            .expect("final");
        assert_eq!(stream.status, Some(204));
        assert!(stream.headers.is_empty());
    }

    #[test]
    fn malformed_heads_are_refused() {
        for head in [
            &[("content-type", "text/html")][..],
            &[(":status", "20")],
            &[(":status", "200"), (":path", "/")],
            &[(":status", "200"), ("Content-Type", "text/html")],
            &[(":status", "200"), ("x", "a\r\nb")],
            &[
                (":status", "200"),
                ("content-type", "a"),
                ("content-type", "b"),
            ],
        ] {
            let mut stream = Stream::new(100, 65_535, Vec::new());
            assert_eq!(
                stream.headers(fields(head), false),
                Err(frame::PROTOCOL_ERROR),
                "{head:?}"
            );
        }
    }

    #[test]
    fn data_before_headers_or_beyond_the_budget_is_refused() {
        let mut stream = Stream::new(4, 65_535, Vec::new());
        assert_eq!(stream.data(b"x", false), Err(frame::PROTOCOL_ERROR));
        stream
            .headers(fields(&[(":status", "200")]), false)
            .expect("head");
        assert_eq!(stream.data(b"12345", false), Err(frame::CANCEL));
        assert!(matches!(
            stream.ending,
            Some(Ending::Failed { retry: false, .. })
        ));
    }

    #[test]
    fn a_content_length_mismatch_is_malformed() {
        let mut stream = Stream::new(100, 65_535, Vec::new());
        stream
            .headers(
                fields(&[(":status", "200"), ("content-length", "5")]),
                false,
            )
            .expect("head");
        assert_eq!(stream.data(b"abc", true), Err(frame::PROTOCOL_ERROR));
    }

    #[test]
    fn request_bodies_respect_both_windows() {
        let mut stream = Stream::new(100, 3, b"hello".to_vec());
        assert_eq!(
            stream.next_chunk(100, 16_384),
            Some((b"hel".to_vec(), false))
        );
        assert_eq!(stream.next_chunk(100, 16_384), None);
        stream.grow(10).expect("grow");
        assert_eq!(stream.next_chunk(1, 16_384), Some((b"l".to_vec(), false)));
        assert_eq!(stream.next_chunk(100, 16_384), Some((b"o".to_vec(), true)));
        assert_eq!(stream.next_chunk(100, 16_384), None);
        assert!(stream.grow(frame::MAX_WINDOW).is_err());
    }

    #[test]
    fn the_receive_window_is_restored_once_more_than_half_is_consumed() {
        let half = i64::from(H2_STREAM_WINDOW) / 2;
        let mut stream = Stream::new(usize::MAX, 65_535, Vec::new());
        assert_eq!(stream.receive(half), Ok(None));
        assert_eq!(stream.receive(1), Ok(Some(half as u32 + 1)));
        assert_eq!(stream.receive(half), Ok(None));
    }

    #[test]
    fn a_server_overrunning_the_receive_window_is_a_flow_control_error() {
        let mut stream = Stream::new(usize::MAX, 65_535, Vec::new());
        stream.ending = Some(Ending::Complete);
        assert_eq!(stream.receive(i64::from(H2_STREAM_WINDOW)), Ok(None));
        assert_eq!(stream.receive(1), Err(frame::FLOW_CONTROL_ERROR));
    }
}
