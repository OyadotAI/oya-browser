/** ISOBMFF box framing: splits bytes into (type, body) pairs, checking every declared size against what is present. */
use crate::constants::{
    FULL_BOX_FLAGS_LEN, FourCc, MAX_BOXES, SIZE_LARGE, SIZE_TO_END, UUID, UUID_LEN,
};
use crate::error::{Error, Result};
use crate::reader::Reader;

/** One ISOBMFF box: its type and the bytes of its body, after the header. */
#[derive(Clone, Copy)]
pub(crate) struct Boxed<'a> {
    pub(crate) kind: FourCc,
    pub(crate) body: &'a [u8],
}

/** Split the first box off `input`, returning it and the bytes that follow it. */
pub(crate) fn split(input: &[u8]) -> Result<(Boxed<'_>, &[u8])> {
    let mut r = Reader::new(input);
    let size32 = r.u32()?;
    let kind = r.fourcc()?;
    let size = match size32 {
        SIZE_TO_END => input.len(),
        SIZE_LARGE => usize::try_from(r.u64()?).map_err(|_| Error::Malformed("box size"))?,
        n => n as usize,
    };
    if kind == UUID {
        r.take(UUID_LEN)?;
    }
    let header = r.position();
    if size < header || size > input.len() {
        return Err(Error::Malformed("box size"));
    }
    Ok((
        Boxed {
            kind,
            body: &input[header..size],
        },
        &input[size..],
    ))
}

/** Every box in `input`, in order, refusing more than MAX_BOXES so a flood of tiny boxes cannot exhaust memory. */
pub(crate) fn children(mut input: &[u8]) -> Result<Vec<Boxed<'_>>> {
    let mut out = Vec::new();
    while !input.is_empty() {
        if out.len() == MAX_BOXES {
            return Err(Error::Limit("box count"));
        }
        let (boxed, rest) = split(input)?;
        out.push(boxed);
        input = rest;
    }
    Ok(out)
}

/** The first box of `kind` among `boxes`, if there is one. */
pub(crate) fn find<'a>(boxes: &[Boxed<'a>], kind: FourCc) -> Option<Boxed<'a>> {
    boxes.iter().copied().find(|b| b.kind == kind)
}

/** The version and flags of a FullBox, and the body after them. */
pub(crate) fn full_box(body: &[u8]) -> Result<(u8, u32, &[u8])> {
    let mut r = Reader::new(body);
    let version = r.byte()?;
    let flags = r.uint(FULL_BOX_FLAGS_LEN)? as u32;
    Ok((version, flags, r.rest()))
}
