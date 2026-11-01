/**
 * Initialization segments: the moov box's tracks, each with its id, timescale, kind, codec string, track-level
 * sample defaults (mvex/trex) and edit-list offset. A media segment's times mean nothing without these, so Media
 * Source reads them once when a SourceBuffer is given its init segment.
 */
use crate::boxes::{Boxed, Cursor, children, find, full_box};
use crate::constants::{
    AOT_SHIFT, AUDIO_ENTRY_FIELDS, AVC1, AVC3, AVCC, DECODER_CONFIG_FIXED, DESC_DECODER_CONFIG,
    DESC_DECODER_SPECIFIC, DESC_ES, EDTS, ELST, ES_FIXED, ES_OCR, ES_SKIP_BYTES,
    ES_STREAM_DEPENDENCE, ES_URL, ESDS, HDLR, Kind, LENGTH_BYTES_MAX, LENGTH_MASK, LENGTH_MORE,
    MDHD, MDIA, MINF, MOOV, MP4A, MVEX, SOUN, STBL, STSD, TKHD, TRAK, TREX, VERSION_LARGE, VIDE,
    VISUAL_ENTRY_FIELDS,
};
use crate::error::{Error, Result};

/** What a track carries, from its handler type. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackKind {
    /** A `vide` handler. */
    Video,
    /** A `soun` handler. */
    Audio,
    /** Any other handler (text, metadata, ...). */
    Other,
}

/** One track of an initialization segment, with everything a media segment needs to be read. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    /** The track id the media segment's tfhd names. */
    pub id: u32,
    /** Units of every decode time and duration on this track. */
    pub timescale: u32,
    /** The handler's kind. */
    pub kind: TrackKind,
    /** The codec string: `avc1.PPCCLL`, `mp4a.40.N`, or the sample entry's four characters. */
    pub codec: String,
    /** The trex default duration, used when a fragment names none. */
    pub default_duration: u32,
    /** The trex default sample size. */
    pub default_size: u32,
    /** The trex default sample flags. */
    pub default_flags: u32,
    /** The first edit's media time, subtracted from presentation times; zero when there is no edit. */
    pub edit_offset: i64,
}

/** The tracks of an initialization segment, in moov order. Fails when there is no moov or no track. */
pub fn parse_init(bytes: &[u8]) -> Result<Vec<Track>> {
    let top = children(bytes)?;
    let moov = find(&top, MOOV).ok_or(Error::Missing(MOOV))?;
    let kids = children(moov.body)?;
    let trex = match find(&kids, MVEX) {
        Some(mvex) => children(mvex.body)?,
        None => Vec::new(),
    };
    let tracks = kids
        .iter()
        .filter(|boxed| boxed.kind == TRAK)
        .map(|trak| parse_trak(trak.body, &trex))
        .collect::<Result<Vec<_>>>()?;
    if tracks.is_empty() {
        return Err(Error::Missing(TRAK));
    }
    Ok(tracks)
}

/** One trak box: id from tkhd, timing from mdia, codec from stsd, defaults from trex, offset from edts. */
fn parse_trak(body: &[u8], trex: &[Boxed<'_>]) -> Result<Track> {
    let kids = children(body)?;
    let id = track_id(required(&kids, TKHD)?.body)?;
    let mdia = children(required(&kids, MDIA)?.body)?;
    let timescale = timescale(required(&mdia, MDHD)?.body)?;
    let kind = handler(required(&mdia, HDLR)?.body)?;
    let minf = children(required(&mdia, MINF)?.body)?;
    let stbl = children(required(&minf, STBL)?.body)?;
    let codec = codec(required(&stbl, STSD)?.body)?;
    let (default_duration, default_size, default_flags) = track_defaults(trex, id)?;
    Ok(Track {
        id,
        timescale,
        kind,
        codec,
        default_duration,
        default_size,
        default_flags,
        edit_offset: edit_offset(&kids)?,
    })
}

/** The first box of a kind inside a container, or Missing naming it. */
fn required<'a>(boxes: &[Boxed<'a>], kind: Kind) -> Result<Boxed<'a>> {
    find(boxes, kind).ok_or(Error::Missing(kind))
}

/** The track id: the 32-bit field after the creation and modification times. */
fn track_id(body: &[u8]) -> Result<u32> {
    let (version, _, payload) = full_box(body)?;
    let mut cursor = Cursor::new(payload, "tkhd");
    cursor.skip(time_fields(version))?;
    cursor.u32()
}

/** The media timescale: the 32-bit field after the creation and modification times. */
fn timescale(body: &[u8]) -> Result<u32> {
    let (version, _, payload) = full_box(body)?;
    let mut cursor = Cursor::new(payload, "mdhd");
    cursor.skip(time_fields(version))?;
    cursor.u32()
}

/** Bytes of creation and modification times: 8 each in version 0, 16 each in version 1 (64-bit). */
fn time_fields(version: u8) -> usize {
    if version == VERSION_LARGE { 16 } else { 8 }
}

/** The track kind from hdlr's handler type, which follows a 4-byte pre_defined field. */
fn handler(body: &[u8]) -> Result<TrackKind> {
    let (_, _, payload) = full_box(body)?;
    let mut cursor = Cursor::new(payload, "hdlr");
    cursor.skip(4)?;
    Ok(match cursor.kind()? {
        VIDE => TrackKind::Video,
        SOUN => TrackKind::Audio,
        _ => TrackKind::Other,
    })
}

/** The codec string of the first sample entry in stsd. */
fn codec(body: &[u8]) -> Result<String> {
    let (_, _, payload) = full_box(body)?;
    let mut cursor = Cursor::new(payload, "stsd");
    cursor.skip(4)?;
    let entries = children(cursor.rest())?;
    let entry = entries.first().ok_or(Error::Missing(STSD))?;
    match entry.kind {
        AVC1 | AVC3 => avc_codec(entry),
        MP4A => aac_codec(entry),
        other => Ok(String::from_utf8_lossy(&other).into_owned()),
    }
}

/** `avc1.PPCCLL` from the avcC record: profile, profile compatibility and level, in hex. */
fn avc_codec(entry: &Boxed<'_>) -> Result<String> {
    let children = entry_children(entry, VISUAL_ENTRY_FIELDS)?;
    let avcc = required(&children, AVCC)?;
    let mut cursor = Cursor::new(avcc.body, "avcC");
    cursor.skip(1)?;
    let profile = cursor.u8()?;
    let compatibility = cursor.u8()?;
    let level = cursor.u8()?;
    Ok(format!(
        "{}.{profile:02X}{compatibility:02X}{level:02X}",
        String::from_utf8_lossy(&entry.kind)
    ))
}

/** `mp4a.40.N` from the esds box's AAC object type. */
fn aac_codec(entry: &Boxed<'_>) -> Result<String> {
    let children = entry_children(entry, AUDIO_ENTRY_FIELDS)?;
    let esds = required(&children, ESDS)?;
    let (_, _, payload) = full_box(esds.body)?;
    Ok(format!("mp4a.40.{}", aac_object_type(payload)?))
}

/** The AAC object type: the high bits of the DecoderSpecificInfo's first byte. */
fn aac_object_type(esds: &[u8]) -> Result<u8> {
    let es = descriptor(esds, DESC_ES)?;
    let flags = *es.get(ES_FIXED - 1).ok_or(Error::Truncated("esds"))?;
    let config = descriptor(es_optionals(es, flags)?, DESC_DECODER_CONFIG)?;
    let specific_at = config
        .get(DECODER_CONFIG_FIXED..)
        .ok_or(Error::Truncated("esds"))?;
    let specific = descriptor(specific_at, DESC_DECODER_SPECIFIC)?;
    let first = specific.first().ok_or(Error::Truncated("esds"))?;
    Ok(first >> AOT_SHIFT)
}

/** The bytes after an ES_Descriptor's fixed fields and whichever optional fields its flags name. */
fn es_optionals(es: &[u8], flags: u8) -> Result<&[u8]> {
    let mut cursor = Cursor::new(es, "esds");
    cursor.skip(ES_FIXED)?;
    if flags & ES_STREAM_DEPENDENCE != 0 {
        cursor.skip(ES_SKIP_BYTES)?;
    }
    if flags & ES_URL != 0 {
        let length = cursor.u8()? as usize;
        cursor.skip(length)?;
    }
    if flags & ES_OCR != 0 {
        cursor.skip(ES_SKIP_BYTES)?;
    }
    Ok(cursor.rest())
}

/** The body of the descriptor at the start of `input`, which must carry `want` as its tag. */
fn descriptor(input: &[u8], want: u8) -> Result<&[u8]> {
    let mut cursor = Cursor::new(input, "descriptor");
    let tag = cursor.u8()?;
    if tag != want {
        return Err(Error::Malformed("descriptor"));
    }
    let length = descriptor_length(&mut cursor)?;
    cursor.take(length)
}

/** A descriptor's variable-length size: seven bits per byte, high bit set while more follow. */
fn descriptor_length(cursor: &mut Cursor<'_>) -> Result<usize> {
    let mut length = 0usize;
    for _ in 0..LENGTH_BYTES_MAX {
        let byte = cursor.u8()?;
        length = (length << 7) | (byte & LENGTH_MASK) as usize;
        if byte & LENGTH_MORE == 0 {
            break;
        }
    }
    Ok(length)
}

/** The child boxes of a sample entry, after its fixed fields. */
fn entry_children<'a>(entry: &Boxed<'a>, fixed: usize) -> Result<Vec<Boxed<'a>>> {
    let rest = entry
        .body
        .get(fixed..)
        .ok_or(Error::Truncated("sample entry"))?;
    children(rest)
}

/** The sample defaults a trex box gives one track: duration, size and flags. Zero when there is no trex. */
fn track_defaults(trex: &[Boxed<'_>], id: u32) -> Result<(u32, u32, u32)> {
    for boxed in trex.iter().filter(|boxed| boxed.kind == TREX) {
        let (_, _, payload) = full_box(boxed.body)?;
        let mut cursor = Cursor::new(payload, "trex");
        if cursor.u32()? != id {
            continue;
        }
        cursor.skip(4)?;
        return Ok((cursor.u32()?, cursor.u32()?, cursor.u32()?));
    }
    Ok((0, 0, 0))
}

/** The media time of the first edit, which presentation times subtract; an empty edit (-1) counts as zero. */
fn edit_offset(kids: &[Boxed<'_>]) -> Result<i64> {
    let Some(edts) = find(kids, EDTS) else {
        return Ok(0);
    };
    let edit_list = children(edts.body)?;
    let Some(elst) = find(&edit_list, ELST) else {
        return Ok(0);
    };
    let (version, _, payload) = full_box(elst.body)?;
    let mut cursor = Cursor::new(payload, "elst");
    cursor.skip(4)?;
    let media_time = if version == VERSION_LARGE {
        cursor.skip(8)?;
        cursor.u64()? as i64
    } else {
        cursor.skip(4)?;
        i64::from(cursor.i32()?)
    };
    Ok(media_time.max(0))
}
