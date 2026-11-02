/**
 * A media resource's bytes as the coordinator has fetched them: sparse chunks at their offsets,
 * the full length once a response told it, and what to fetch next. The native player's resource
 * loader reads from here and records the first byte it is missing; the coordinator fills that
 * with a byte-range request through Relay. Nothing here touches the network or the OS.
 */
use std::collections::BTreeMap;
use std::sync::Arc;

/// Bytes one range request asks for: a couple of seconds of HD video, small enough that one
/// fetch never holds the coordinator long.
pub const CHUNK: u64 = 2 * 1024 * 1024;
/// Bytes kept per resource; beyond it the chunks farthest from the reader are dropped.
pub const RETAINED: usize = 192 * 1024 * 1024;

/** Sparse fetched bytes of one resource. */
#[derive(Debug, Default)]
pub struct Bytes {
    /// Fetched chunks by start offset; chunks never overlap.
    chunks: BTreeMap<u64, Arc<Vec<u8>>>,
    /// Total length, known after the first response (Content-Range or a whole body).
    pub length: Option<u64>,
    /// The response's media type, which the loader maps to a container type.
    pub mime: String,
    /// The fetch failed for good: the player reports a network or format error.
    pub failed: Option<(u16, String)>,
    /// Bytes currently held, bounded by RETAINED.
    held: usize,
}

impl Bytes {
    /** Every byte of a resource known up front (a Blob). */
    pub fn whole(bytes: Vec<u8>, mime: &str) -> Self {
        let mut out = Self {
            length: Some(bytes.len() as u64),
            mime: mime.into(),
            ..Self::default()
        };
        out.held = bytes.len();
        out.chunks.insert(0, Arc::new(bytes));
        out
    }
    /** The contiguous bytes held from `offset`, up to `limit` of them; empty when missing. */
    pub fn read(&self, offset: u64, limit: usize) -> &[u8] {
        let Some((start, chunk)) = self.chunks.range(..=offset).next_back() else {
            return &[];
        };
        let skip = (offset - start) as usize;
        if skip >= chunk.len() {
            return &[];
        }
        let end = chunk.len().min(skip + limit);
        &chunk[skip..end]
    }
    /** Whether the byte at `offset` is held (or lies past the known end). */
    pub fn has(&self, offset: u64) -> bool {
        self.length.is_some_and(|length| offset >= length) || !self.read(offset, 1).is_empty()
    }
    /** The range to fetch so the reader at `offset` can continue: up to CHUNK bytes, stopping
    at the next chunk already held and at the known end. None when nothing is missing there. */
    pub fn plan(&self, offset: u64) -> Option<(u64, u64)> {
        if self.has(offset) {
            return None;
        }
        let mut end = offset + CHUNK;
        if let Some((next, _)) = self.chunks.range(offset + 1..).next() {
            end = end.min(*next);
        }
        if let Some(length) = self.length {
            end = end.min(length);
        }
        (end > offset).then_some((offset, end - 1))
    }
    /** Store fetched bytes at `offset`, trimming any overlap with chunks already held, then drop
    the chunks farthest from `reader` while over the retention bound. */
    pub fn insert(&mut self, offset: u64, bytes: Vec<u8>, reader: u64) {
        let mut start = offset;
        let mut bytes = bytes;
        if let Some((held_start, chunk)) = self.chunks.range(..=offset).next_back() {
            let held_end = held_start + chunk.len() as u64;
            if held_end > start {
                let cut = ((held_end - start) as usize).min(bytes.len());
                bytes.drain(..cut);
                start = held_end;
            }
        }
        if let Some((next, _)) = self.chunks.range(start..).next() {
            bytes.truncate((next - start) as usize);
        }
        if bytes.is_empty() {
            return;
        }
        self.held += bytes.len();
        self.chunks.insert(start, Arc::new(bytes));
        self.evict(reader);
    }
    /** Drop the chunk farthest from the reader until the held bytes fit. */
    fn evict(&mut self, reader: u64) {
        while self.held > RETAINED && self.chunks.len() > 1 {
            let farthest = self
                .chunks
                .keys()
                .copied()
                .max_by_key(|start| start.abs_diff(reader))
                .expect("chunks are not empty");
            if let Some(chunk) = self.chunks.remove(&farthest) {
                self.held -= chunk.len();
            }
        }
    }
    /** Byte spans held, for buffered-range estimates. */
    pub fn spans(&self) -> Vec<(u64, u64)> {
        let mut out: Vec<(u64, u64)> = Vec::new();
        for (start, chunk) in &self.chunks {
            let end = start + chunk.len() as u64;
            match out.last_mut() {
                Some(last) if last.1 == *start => last.1 = end,
                _ => out.push((*start, end)),
            }
        }
        out
    }
}

/** `bytes a-b/total` from a Content-Range header: the first byte and the total length. */
pub fn content_range(value: &str) -> Option<(u64, Option<u64>)> {
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (span, total) = rest.split_once('/')?;
    let (start, _) = span.trim().split_once('-')?;
    let total = match total.trim() {
        "*" => None,
        total => Some(total.parse().ok()?),
    };
    Some((start.trim().parse().ok()?, total))
}
