/**
 * One HTTP/2 connection's protocol state: TLS, HPACK, streams and flow control. It is pure
 * bookkeeping over bytes; `Session` decides who reads the socket and who waits.
 */
use super::frame::{self, Frame};
use super::hpack::{self, Decoder};
use super::stream::{Ending, Stream};
use crate::network::constants::{
    H2_CONNECTION_INCREMENT, H2_CONTINUATIONS, H2_FRAME_BYTES, H2_HEADER_LIST_BYTES,
    H2_HEADER_TABLE_BYTES, H2_STREAMS,
};
use crate::protocol::{EngineError, EngineResult};
use rustls::ClientConnection;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Instant;

/** The receive window Relay grants the whole connection after its first WINDOW_UPDATE. */
const CONNECTION_WINDOW: i64 = frame::DEFAULT_WINDOW + H2_CONNECTION_INCREMENT as i64;

/** A header block still waiting for CONTINUATION frames. */
struct Block {
    stream: u32,
    bytes: Vec<u8>,
    end_stream: bool,
    continuations: usize,
}

/// Everything behind the session lock.
pub(super) struct State {
    tls: ClientConnection,
    /** Writes go out on this handle; reads arrive raw from the session's clone. */
    socket: TcpStream,
    decoder: Decoder,
    /** Decrypted bytes not yet framed; at most one frame plus one read. */
    inbox: Vec<u8>,
    streams: HashMap<u32, Stream>,
    next_stream: u32,
    /** Whether some caller is reading the socket right now. */
    pub reading: bool,
    peer_streams: usize,
    peer_window: i64,
    send_window: i64,
    receive_window: i64,
    /** Bytes received since the last connection WINDOW_UPDATE. */
    unacknowledged: i64,
    block: Option<Block>,
    /** Highest stream the server will process, after GOAWAY. */
    goaway: Option<u32>,
    /** The connection failed; no stream can make progress. */
    closed: bool,
    /** Deadline of the operation in progress, for socket writes. */
    deadline: Instant,
    /** When a stream last finished or opened, for idle expiry. */
    pub used: Instant,
}

impl State {
    /// Takes over a negotiated TLS session and queues the client preface.
    pub(super) fn new(
        tls: ClientConnection,
        socket: TcpStream,
        deadline: Instant,
    ) -> EngineResult<Self> {
        let mut state = Self {
            tls,
            socket,
            decoder: Decoder::new(H2_HEADER_TABLE_BYTES),
            inbox: Vec::new(),
            streams: HashMap::new(),
            next_stream: 1,
            reading: false,
            peer_streams: H2_STREAMS,
            peer_window: frame::DEFAULT_WINDOW,
            send_window: frame::DEFAULT_WINDOW,
            receive_window: CONNECTION_WINDOW,
            unacknowledged: 0,
            block: None,
            goaway: None,
            closed: false,
            deadline,
            used: Instant::now(),
        };
        state.send(&frame::preface());
        state.flush()?;
        Ok(state)
    }

    /// Whether a new stream may still be opened on this connection.
    pub(super) fn accepting(&self) -> bool {
        !self.closed && self.goaway.is_none() && self.next_stream < frame::MAX_WINDOW as u32
    }

    /// Whether another stream fits under both Relay's and the server's concurrency limits.
    pub(super) fn has_capacity(&self) -> bool {
        self.streams.len() < self.peer_streams.min(H2_STREAMS)
    }

    /// Opens a stream: HEADERS (split into CONTINUATION as needed), then as much of `body`
    /// as flow control allows. Ok(None) means the connection can take no new streams. A `head`
    /// stream expects no response body.
    pub(super) fn open(
        &mut self,
        fields: &[(&str, &str)],
        weight: u8,
        body: &[u8],
        limit: usize,
        head: bool,
        deadline: Instant,
    ) -> EngineResult<Option<u32>> {
        if !self.accepting() {
            return Ok(None);
        }
        self.deadline = deadline;
        let id = self.next_stream;
        self.next_stream += 2;
        let block = hpack::encode(fields);
        self.write_headers(id, &block, weight, body.is_empty());
        let mut stream = Stream::new(limit, self.peer_window, body.to_vec());
        stream.head = head;
        self.streams.insert(id, stream);
        self.send_pending(id);
        self.used = Instant::now();
        self.flush()?;
        Ok(Some(id))
    }

    /// Removes and returns a finished stream's outcome, if it has one.
    pub(super) fn take(&mut self, id: u32) -> Option<(Stream, Ending)> {
        let ending = self.streams.get_mut(&id)?.ending.take()?;
        self.used = Instant::now();
        self.streams.remove(&id).map(|stream| (stream, ending))
    }

    /// Abandons a stream whose caller gave up, telling the server to stop sending it.
    pub(super) fn cancel(&mut self, id: u32) {
        if self.streams.remove(&id).is_some() && !self.closed {
            self.reset(id, frame::CANCEL);
            let _ = self.flush();
        }
    }

    /// Feeds raw socket bytes: TLS records in, frames dispatched, replies flushed. Empty input
    /// means the peer closed the socket.
    pub(super) fn feed(&mut self, mut raw: &[u8], deadline: Instant) {
        self.deadline = deadline;
        if raw.is_empty() {
            return self.close(true);
        }
        while !raw.is_empty() && !self.closed {
            if !matches!(self.tls.read_tls(&mut raw), Ok(count) if count > 0) || !self.decrypt() {
                return self.close(true);
            }
        }
        if self.flush().is_err() {
            self.close(true);
        }
    }

    /// Fails the connection after a read error or timeout outside the protocol.
    pub(super) fn abandon(&mut self) {
        self.close(true);
    }

    /** Processes TLS records and frames everything decrypted; false if TLS failed or closed. */
    fn decrypt(&mut self) -> bool {
        let Ok(io) = self.tls.process_new_packets() else {
            return false;
        };
        let mut plaintext = vec![0; io.plaintext_bytes_to_read()];
        if self.tls.reader().read_exact(&mut plaintext).is_err() {
            return false;
        }
        self.inbox.extend_from_slice(&plaintext);
        self.frames();
        !io.peer_has_closed() || self.closed
    }

    /** Dispatches every complete frame in the inbox, keeping a trailing partial frame. */
    fn frames(&mut self) {
        let inbox = std::mem::take(&mut self.inbox);
        let mut used = 0;
        while !self.closed {
            match frame::parse(&inbox[used..]) {
                Ok(Some((frame, size))) => {
                    used += size;
                    if let Err(code) = self.dispatch(&frame) {
                        self.fail(code);
                    }
                }
                Ok(None) => break,
                Err(code) => self.fail(code),
            }
        }
        if !self.closed {
            self.inbox = inbox[used..].to_vec();
        }
    }

    /** Section 6: one frame. An Err is a connection error with that code. */
    fn dispatch(&mut self, frame: &Frame<'_>) -> Result<(), u32> {
        if self.block.is_some() && frame.kind != frame::CONTINUATION {
            return Err(frame::PROTOCOL_ERROR);
        }
        match frame.kind {
            frame::DATA => self.data(frame),
            frame::HEADERS => self.headers(frame),
            frame::CONTINUATION => self.continuation(frame),
            frame::RST_STREAM => self.rst_stream(frame),
            frame::SETTINGS => self.settings(frame),
            frame::PING => self.ping(frame),
            frame::GOAWAY => self.goaway(frame),
            frame::WINDOW_UPDATE => self.window_update(frame),
            frame::PUSH_PROMISE => Err(frame::PROTOCOL_ERROR),
            frame::PRIORITY if frame.payload.len() != 5 => Err(frame::FRAME_SIZE_ERROR),
            _ => Ok(()),
        }
    }

    /** DATA: connection flow control always; the body only for a stream that has headers. */
    fn data(&mut self, frame: &Frame<'_>) -> Result<(), u32> {
        if frame.stream == 0 {
            return Err(frame::PROTOCOL_ERROR);
        }
        let size = frame.payload.len() as i64;
        self.receive_window -= size;
        if self.receive_window < 0 {
            return Err(frame::FLOW_CONTROL_ERROR);
        }
        self.acknowledge(size);
        let content = frame::content(frame)?;
        let end = frame.flags & frame::END_STREAM != 0;
        let Some(stream) = self.streams.get_mut(&frame.stream) else {
            return Ok(());
        };
        let outcome = stream
            .data(content, end)
            .and_then(|()| stream.receive(size));
        match outcome {
            Ok(Some(increment)) => self.window_update_for(frame.stream, increment),
            Ok(None) => {}
            Err(code) => self.reset(frame.stream, code),
        }
        Ok(())
    }

    /** Replenishes the connection window once more than half of it has been used, as Chrome does. */
    fn acknowledge(&mut self, size: i64) {
        self.unacknowledged += size;
        if self.unacknowledged > CONNECTION_WINDOW / 2 {
            self.window_update_for(0, self.unacknowledged as u32);
            self.receive_window += self.unacknowledged;
            self.unacknowledged = 0;
        }
    }

    /** Queues a WINDOW_UPDATE granting `increment` more bytes on `stream` (0: the connection). */
    fn window_update_for(&mut self, stream: u32, increment: u32) {
        let mut bytes = Vec::new();
        frame::write(
            &mut bytes,
            frame::WINDOW_UPDATE,
            0,
            stream,
            &increment.to_be_bytes(),
        );
        self.send(&bytes);
    }

    /** HEADERS starts a block; it completes now or after CONTINUATION frames. */
    fn headers(&mut self, frame: &Frame<'_>) -> Result<(), u32> {
        if frame.stream == 0 {
            return Err(frame::PROTOCOL_ERROR);
        }
        let content = frame::content(frame)?;
        self.block = Some(Block {
            stream: frame.stream,
            bytes: content.to_vec(),
            end_stream: frame.flags & frame::END_STREAM != 0,
            continuations: 0,
        });
        self.end_block(frame.flags)
    }

    /** CONTINUATION extends the open block, bounded in bytes and in frames. */
    fn continuation(&mut self, frame: &Frame<'_>) -> Result<(), u32> {
        let block = self
            .block
            .as_mut()
            .filter(|block| block.stream == frame.stream)
            .ok_or(frame::PROTOCOL_ERROR)?;
        block.continuations += 1;
        block.bytes.extend_from_slice(frame.payload);
        if block.continuations > H2_CONTINUATIONS || block.bytes.len() > H2_HEADER_LIST_BYTES {
            return Err(frame::ENHANCE_YOUR_CALM);
        }
        self.end_block(frame.flags)
    }

    /** With END_HEADERS, decodes the block (always, to keep HPACK in sync) and delivers it. */
    fn end_block(&mut self, flags: u8) -> Result<(), u32> {
        if flags & frame::END_HEADERS == 0 {
            return Ok(());
        }
        let block = self.block.take().ok_or(frame::PROTOCOL_ERROR)?;
        if block.bytes.len() > H2_HEADER_LIST_BYTES {
            return Err(frame::ENHANCE_YOUR_CALM);
        }
        let fields = self
            .decoder
            .decode(&block.bytes, H2_HEADER_LIST_BYTES)
            .map_err(|error| match error {
                hpack::Error::Compression => frame::COMPRESSION_ERROR,
                hpack::Error::TooLarge => frame::ENHANCE_YOUR_CALM,
            })?;
        if let Some(stream) = self.streams.get_mut(&block.stream)
            && let Err(code) = stream.headers(fields, block.end_stream)
        {
            self.reset(block.stream, code);
        }
        Ok(())
    }

    /** RST_STREAM ends one stream; REFUSED_STREAM means it was never processed. */
    fn rst_stream(&mut self, frame: &Frame<'_>) -> Result<(), u32> {
        if frame.payload.len() != 4 {
            return Err(frame::FRAME_SIZE_ERROR);
        }
        if frame.stream == 0 {
            return Err(frame::PROTOCOL_ERROR);
        }
        let code = frame::u32_at(frame.payload, 0);
        if let Some(stream) = self.streams.get_mut(&frame.stream) {
            stream.fail(code == frame::REFUSED_STREAM, reset_error());
        }
        Ok(())
    }

    /** SETTINGS: apply the ones that constrain Relay, then acknowledge. */
    fn settings(&mut self, frame: &Frame<'_>) -> Result<(), u32> {
        if frame.stream != 0 {
            return Err(frame::PROTOCOL_ERROR);
        }
        if frame.flags & frame::ACK != 0 {
            return if frame.payload.is_empty() {
                Ok(())
            } else {
                Err(frame::FRAME_SIZE_ERROR)
            };
        }
        if !frame.payload.len().is_multiple_of(6) {
            return Err(frame::FRAME_SIZE_ERROR);
        }
        for entry in frame.payload.chunks_exact(6) {
            let value = frame::u32_at(entry, 2);
            self.setting(u16::from_be_bytes([entry[0], entry[1]]), value)?;
        }
        let mut bytes = Vec::new();
        frame::write(&mut bytes, frame::SETTINGS, frame::ACK, 0, &[]);
        self.send(&bytes);
        let ids: Vec<u32> = self.streams.keys().copied().collect();
        ids.into_iter().for_each(|id| self.send_pending(id));
        Ok(())
    }

    /** One setting; unknown identifiers are ignored (section 6.5.2). */
    fn setting(&mut self, id: u16, value: u32) -> Result<(), u32> {
        match id {
            frame::SETTINGS_MAX_CONCURRENT_STREAMS => self.peer_streams = value as usize,
            frame::SETTINGS_INITIAL_WINDOW_SIZE => {
                if i64::from(value) > frame::MAX_WINDOW {
                    return Err(frame::FLOW_CONTROL_ERROR);
                }
                let delta = i64::from(value) - self.peer_window;
                self.peer_window = i64::from(value);
                for stream in self.streams.values_mut() {
                    stream.grow(delta).map_err(|_| frame::FLOW_CONTROL_ERROR)?;
                }
            }
            frame::SETTINGS_MAX_FRAME_SIZE => {
                if !(H2_FRAME_BYTES as u32..=frame::MAX_FRAME_LIMIT).contains(&value) {
                    return Err(frame::PROTOCOL_ERROR);
                }
            }
            frame::SETTINGS_ENABLE_PUSH if value > 1 => return Err(frame::PROTOCOL_ERROR),
            _ => {}
        }
        Ok(())
    }

    /** PING is answered with the same eight bytes. */
    fn ping(&mut self, frame: &Frame<'_>) -> Result<(), u32> {
        if frame.payload.len() != 8 {
            return Err(frame::FRAME_SIZE_ERROR);
        }
        if frame.stream != 0 {
            return Err(frame::PROTOCOL_ERROR);
        }
        if frame.flags & frame::ACK == 0 {
            let mut bytes = Vec::new();
            frame::write(&mut bytes, frame::PING, frame::ACK, 0, frame.payload);
            self.send(&bytes);
        }
        Ok(())
    }

    /** GOAWAY: streams above the last processed one may be retried elsewhere. */
    fn goaway(&mut self, frame: &Frame<'_>) -> Result<(), u32> {
        if frame.payload.len() < 8 {
            return Err(frame::FRAME_SIZE_ERROR);
        }
        let last = frame::u32_at(frame.payload, 0) & 0x7fff_ffff;
        self.goaway = Some(self.goaway.map_or(last, |previous| previous.min(last)));
        for (id, stream) in &mut self.streams {
            if *id > last {
                stream.fail(true, reset_error());
            }
        }
        Ok(())
    }

    /** WINDOW_UPDATE widens a send window, then pending request bodies move. */
    fn window_update(&mut self, frame: &Frame<'_>) -> Result<(), u32> {
        if frame.payload.len() != 4 {
            return Err(frame::FRAME_SIZE_ERROR);
        }
        let increment = i64::from(frame::u32_at(frame.payload, 0) & 0x7fff_ffff);
        if frame.stream == 0 {
            self.send_window += increment;
            if increment == 0 || self.send_window > frame::MAX_WINDOW {
                return Err(if increment == 0 {
                    frame::PROTOCOL_ERROR
                } else {
                    frame::FLOW_CONTROL_ERROR
                });
            }
            let ids: Vec<u32> = self.streams.keys().copied().collect();
            ids.into_iter().for_each(|id| self.send_pending(id));
            return Ok(());
        }
        let Some(stream) = self.streams.get_mut(&frame.stream) else {
            return Ok(());
        };
        if increment == 0 || stream.grow(increment).is_err() {
            let code = if increment == 0 {
                frame::PROTOCOL_ERROR
            } else {
                frame::FLOW_CONTROL_ERROR
            };
            self.reset(frame.stream, code);
            return Ok(());
        }
        self.send_pending(frame.stream);
        Ok(())
    }

    /** HEADERS with Chrome's priority fields (exclusive, no dependency), then CONTINUATIONs. */
    fn write_headers(&mut self, id: u32, block: &[u8], weight: u8, end_stream: bool) {
        let mut first = vec![0x80, 0, 0, 0, weight];
        let room = H2_FRAME_BYTES - first.len();
        let (head, mut rest) = block.split_at(block.len().min(room));
        first.extend_from_slice(head);
        let mut flags = frame::PRIORITY_FLAG | if end_stream { frame::END_STREAM } else { 0 };
        if rest.is_empty() {
            flags |= frame::END_HEADERS;
        }
        let mut bytes = Vec::new();
        frame::write(&mut bytes, frame::HEADERS, flags, id, &first);
        while !rest.is_empty() {
            let (part, tail) = rest.split_at(rest.len().min(H2_FRAME_BYTES));
            let flags = if tail.is_empty() {
                frame::END_HEADERS
            } else {
                0
            };
            frame::write(&mut bytes, frame::CONTINUATION, flags, id, part);
            rest = tail;
        }
        self.send(&bytes);
    }

    /** Sends as much of a stream's request body as both windows allow. */
    fn send_pending(&mut self, id: u32) {
        let Some(stream) = self.streams.get_mut(&id) else {
            return;
        };
        let mut bytes = Vec::new();
        while let Some((chunk, end)) = stream.next_chunk(self.send_window, H2_FRAME_BYTES) {
            self.send_window -= chunk.len() as i64;
            let flags = if end { frame::END_STREAM } else { 0 };
            frame::write(&mut bytes, frame::DATA, flags, id, &chunk);
        }
        self.send(&bytes);
    }

    /** RST_STREAM for one stream; its caller sees a final error. */
    fn reset(&mut self, id: u32, code: u32) {
        if let Some(stream) = self.streams.get_mut(&id) {
            stream.fail(false, reset_error());
        }
        let mut bytes = Vec::new();
        frame::write(&mut bytes, frame::RST_STREAM, 0, id, &code.to_be_bytes());
        self.send(&bytes);
    }

    /** A connection error: GOAWAY with `code`, then every stream fails. */
    fn fail(&mut self, code: u32) {
        // Relay accepts no server-initiated streams, so the last one it processed is 0.
        let mut payload = 0u32.to_be_bytes().to_vec();
        payload.extend_from_slice(&code.to_be_bytes());
        let mut bytes = Vec::new();
        frame::write(&mut bytes, frame::GOAWAY, 0, 0, &payload);
        self.send(&bytes);
        let _ = self.flush();
        self.close(false);
    }

    /** Marks the connection dead; unanswered streams may retry when `retry` allows it. */
    fn close(&mut self, retry: bool) {
        self.closed = true;
        self.inbox.clear();
        for stream in self.streams.values_mut() {
            stream.fail(retry, reset_error());
        }
    }

    /** Queues plaintext into TLS. Rustls buffers it; `flush` writes it to the socket. */
    fn send(&mut self, bytes: &[u8]) {
        if !bytes.is_empty() && self.tls.writer().write_all(bytes).is_err() {
            self.closed = true;
        }
    }

    /** Writes queued TLS records within the current operation's deadline. */
    fn flush(&mut self) -> EngineResult<()> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(reset_error());
        }
        self.socket
            .set_write_timeout(Some(remaining))
            .map_err(|_| reset_error())?;
        while self.tls.wants_write() {
            if self.tls.write_tls(&mut self.socket).is_err() {
                self.closed = true;
                return Err(reset_error());
            }
        }
        Ok(())
    }
}

/** A stream the server reset or a connection that failed. */
pub(super) fn reset_error() -> EngineError {
    EngineError::new(
        "network_error",
        "The HTTP/2 connection or stream was reset before the response completed.",
        "Retry the destination.",
    )
}
