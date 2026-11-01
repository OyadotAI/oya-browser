/**
 * Box framing for ISO BMFF: a box's size and type header, its payload, and a bounds-checked cursor over fixed-width
 * fields. Every read checks the length it needs first, so a cut-short box returns Truncated and never panics.
 */
use crate::constants::{HEADER, HEADER_LARGE, Kind, SIZE_LARGE, SIZE_TO_END};
use crate::error::{Error, Result};

/** One box: its type and its payload, with the header already removed. */
#[derive(Clone, Copy, Debug)]
pub(crate) struct Boxed<'a> {
    pub kind: Kind,
    pub body: &'a [u8],
}

/** Split the first box off `input`, returning it and the bytes after it. */
pub(crate) fn split(input: &[u8]) -> Result<(Boxed<'_>, &[u8])> {
    let mut cursor = Cursor::new(input, "box header");
    let size = cursor.u32()?;
    let kind = cursor.kind()?;
    let (header, total) = match size {
        SIZE_TO_END => (HEADER, input.len()),
        SIZE_LARGE => (HEADER_LARGE, large_size(input)?),
        size => (HEADER, size as usize),
    };
    if total < header || total > input.len() {
        return Err(Error::Truncated("box"));
    }
    let boxed = Boxed {
        kind,
        body: &input[header..total],
    };
    Ok((boxed, &input[total..]))
}

/** The 64-bit size that follows a large-size box header. */
fn large_size(input: &[u8]) -> Result<usize> {
    let mut cursor = Cursor::new(&input[HEADER..], "large box size");
    usize::try_from(cursor.u64()?).map_err(|_| Error::Malformed("box size"))
}

/** Every box in `input`, in order. */
pub(crate) fn children(mut input: &[u8]) -> Result<Vec<Boxed<'_>>> {
    let mut boxes = Vec::new();
    while !input.is_empty() {
        let (boxed, rest) = split(input)?;
        boxes.push(boxed);
        input = rest;
    }
    Ok(boxes)
}

/** The first box of a type, if any. */
pub(crate) fn find<'a>(boxes: &[Boxed<'a>], kind: Kind) -> Option<Boxed<'a>> {
    boxes.iter().copied().find(|boxed| boxed.kind == kind)
}

/** A full box's version, flags and remaining payload. */
pub(crate) fn full_box(body: &[u8]) -> Result<(u8, u32, &[u8])> {
    let mut cursor = Cursor::new(body, "full box");
    let version = cursor.u8()?;
    let flags = cursor.u24()?;
    Ok((version, flags, cursor.rest()))
}

/** A forward-only reader over a box's fields. Each read says what it was reading when it ran out. */
#[derive(Clone, Copy)]
pub(crate) struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
    what: &'static str,
}

impl<'a> Cursor<'a> {
    /** A cursor at the start of `data`, labelled with what it reads for errors. */
    pub(crate) fn new(data: &'a [u8], what: &'static str) -> Self {
        Self { data, pos: 0, what }
    }
    /** The next `count` bytes, advancing past them. */
    pub(crate) fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(count)
            .ok_or(Error::Truncated(self.what))?;
        let bytes = self
            .data
            .get(self.pos..end)
            .ok_or(Error::Truncated(self.what))?;
        self.pos = end;
        Ok(bytes)
    }
    /** Skip `count` bytes. */
    pub(crate) fn skip(&mut self, count: usize) -> Result<()> {
        self.take(count).map(|_| ())
    }
    /** One byte. */
    pub(crate) fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    /** A 24-bit big-endian value. */
    pub(crate) fn u24(&mut self) -> Result<u32> {
        let bytes = self.take(3)?;
        Ok(u32::from_be_bytes([0, bytes[0], bytes[1], bytes[2]]))
    }
    /** A 32-bit big-endian value. */
    pub(crate) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.array()?))
    }
    /** A signed 32-bit big-endian value. */
    pub(crate) fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.array()?))
    }
    /** A 64-bit big-endian value. */
    pub(crate) fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.array()?))
    }
    /** A box type. */
    pub(crate) fn kind(&mut self) -> Result<Kind> {
        self.array()
    }
    /** The unread bytes. */
    pub(crate) fn rest(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }
    /** A fixed-size array of the next bytes. */
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.take(N)?;
        bytes.try_into().map_err(|_| Error::Truncated(self.what))
    }
}
