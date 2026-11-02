/**
 * Media segments: each moof's track fragments, read into samples. A sample's bytes sit in the segment at an
 * offset that trun and tfhd resolve, and its presentation time is its decode time plus the composition offset,
 * minus the track's edit offset. Offsets are into the segment as given, so the caller can slice the sample out.
 */
use crate::boxes::{Boxed, Cursor, children, find, full_box, split};
use crate::constants::{
    MDAT, MOOF, SAMPLE_LIMIT, SAMPLE_NON_SYNC, SAMPLES_PER_SEGMENT, TFDT, TFHD, TFHD_BASE_IS_MOOF,
    TFHD_BASE_OFFSET, TFHD_DEFAULT_DURATION, TFHD_DEFAULT_FLAGS, TFHD_DEFAULT_SIZE,
    TFHD_SAMPLE_DESCRIPTION, TRAF, TRUN, TRUN_COMPOSITION, TRUN_DATA_OFFSET, TRUN_DURATION,
    TRUN_FIRST_FLAGS, TRUN_FLAGS, TRUN_SIZE, VERSION_LARGE,
};
use crate::error::{Error, Result};
use crate::init::Track;

/** One sample of a media segment: where its bytes are and when it plays. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    /** The track it belongs to. */
    pub track: u32,
    /** Where its bytes start in the segment. */
    pub offset: usize,
    /** How many bytes it takes. */
    pub size: usize,
    /** Decode time, in the track's timescale. */
    pub decode_time: u64,
    /** Presentation time, in the track's timescale, with the edit offset removed. */
    pub pts: i64,
    /** Duration, in the track's timescale. */
    pub duration: u32,
    /** Whether a decoder can start here without earlier samples. */
    pub sync: bool,
}

/**
 * The samples of every fragment in a media segment, in file order. `tracks` comes from the init segment. A segment
 * yields at most `min(bytes.len(), SAMPLES_PER_SEGMENT)` samples: every sample must name at least one byte of the
 * segment, so a count beyond the segment's own length is hostile, and the cap bounds the rest. Samples must not
 * overlap, and together they may not exceed the segment's media data: the caller copies those bytes, so this bounds
 * what one append can make the player hold. Refusal is an error.
 */
pub fn parse_segment(bytes: &[u8], tracks: &[Track]) -> Result<Vec<Sample>> {
    let budget = bytes.len().min(SAMPLES_PER_SEGMENT);
    let mut samples = Vec::new();
    let mut media_bytes = 0usize;
    let mut rest = bytes;
    while !rest.is_empty() {
        let moof_start = bytes.len() - rest.len();
        let (boxed, after) = split(rest)?;
        if boxed.kind == MOOF {
            let room = budget - samples.len();
            samples.extend(parse_moof(bytes, boxed, moof_start, tracks, room)?);
        } else if boxed.kind == MDAT {
            media_bytes += boxed.body.len();
        }
        rest = after;
    }
    check_disjoint(&samples)?;
    let claimed: usize = samples.iter().map(|sample| sample.size).sum();
    if claimed > media_bytes {
        return Err(Error::Malformed("sample bytes past mdat"));
    }
    Ok(samples)
}

/** Samples in file order must each start at or after the end of the one before: overlapping ranges would let one
 * append copy the same bytes many times over. */
fn check_disjoint(samples: &[Sample]) -> Result<()> {
    for pair in samples.windows(2) {
        if pair[1].offset < pair[0].offset + pair[0].size {
            return Err(Error::Malformed("overlapping samples"));
        }
    }
    Ok(())
}

/** One moof: each traf in order; a traf without an explicit base continues after the previous one. */
fn parse_moof(
    bytes: &[u8],
    moof: Boxed<'_>,
    moof_start: usize,
    tracks: &[Track],
    room: usize,
) -> Result<Vec<Sample>> {
    let mut samples = Vec::new();
    let mut previous_end = None;
    for traf in children(moof.body)?
        .iter()
        .filter(|boxed| boxed.kind == TRAF)
    {
        let (run, end) = parse_traf(
            bytes,
            traf,
            moof_start,
            previous_end,
            tracks,
            room - samples.len(),
        )?;
        previous_end = Some(end);
        samples.extend(run);
    }
    Ok(samples)
}

/** The samples of one traf, and the offset just past its data. */
fn parse_traf(
    bytes: &[u8],
    traf: &Boxed<'_>,
    moof_start: usize,
    previous_end: Option<usize>,
    tracks: &[Track],
    room: usize,
) -> Result<(Vec<Sample>, usize)> {
    let kids = children(traf.body)?;
    let header = find(&kids, TFHD).ok_or(Error::Missing(TFHD))?;
    let fields = TrackFragment::read(header.body)?;
    let track = tracks
        .iter()
        .find(|track| track.id == fields.track)
        .ok_or(Error::Malformed("track fragment for an unknown track"))?;
    let fallback = match previous_end {
        Some(end) if !fields.base_is_moof => end,
        _ => moof_start,
    };
    let base = fields.base_or(fallback);
    let decode_time = match find(&kids, TFDT) {
        Some(tfdt) => decode_time(tfdt.body)?,
        None => 0,
    };
    let mut reader = RunReader {
        data_pos: base,
        decode_time,
        end: base,
        samples: Vec::new(),
        room,
    };
    for run in kids.iter().filter(|boxed| boxed.kind == TRUN) {
        reader.run(bytes, run.body, &fields, track, moof_start)?;
    }
    Ok((reader.samples, reader.end))
}

/** The fields a tfhd sets for its track fragment; each optional one falls back to the trex default. */
struct TrackFragment {
    track: u32,
    base: Option<u64>,
    base_is_moof: bool,
    default_duration: Option<u32>,
    default_size: Option<u32>,
    default_flags: Option<u32>,
}

impl TrackFragment {
    /** Read a tfhd: the track id, then each optional field its flags name, in order. */
    fn read(body: &[u8]) -> Result<Self> {
        let (_, flags, payload) = full_box(body)?;
        let mut cursor = Cursor::new(payload, "tfhd");
        let track = cursor.u32()?;
        let base = if flags & TFHD_BASE_OFFSET != 0 {
            Some(cursor.u64()?)
        } else {
            None
        };
        if flags & TFHD_SAMPLE_DESCRIPTION != 0 {
            cursor.skip(4)?;
        }
        let default_duration = optional(flags, TFHD_DEFAULT_DURATION, &mut cursor)?;
        let default_size = optional(flags, TFHD_DEFAULT_SIZE, &mut cursor)?;
        let default_flags = optional(flags, TFHD_DEFAULT_FLAGS, &mut cursor)?;
        Ok(Self {
            track,
            base,
            base_is_moof: flags & TFHD_BASE_IS_MOOF != 0,
            default_duration,
            default_size,
            default_flags,
        })
    }
    /** The explicit base, or the fallback the caller computed for a traf without one. */
    fn base_or(&self, fallback: usize) -> usize {
        self.base.map_or(fallback, |base| base as usize)
    }
}

/** A 32-bit tfhd field, read only when its flag bit is set. */
fn optional(flags: u32, bit: u32, cursor: &mut Cursor<'_>) -> Result<Option<u32>> {
    if flags & bit != 0 {
        cursor.u32().map(Some)
    } else {
        Ok(None)
    }
}

/** The base media decode time of a traf, 64-bit in version 1. */
fn decode_time(body: &[u8]) -> Result<u64> {
    let (version, _, payload) = full_box(body)?;
    let mut cursor = Cursor::new(payload, "tfdt");
    if version == VERSION_LARGE {
        cursor.u64()
    } else {
        cursor.u32().map(u64::from)
    }
}

/** Running state across the truns of one traf: where the next sample's bytes start and the decode clock. */
struct RunReader {
    data_pos: usize,
    decode_time: u64,
    end: usize,
    samples: Vec<Sample>,
    /// How many more samples the segment's budget allows this traf to yield.
    room: usize,
}

impl RunReader {
    /** One trun: its data offset (if any), then each sample's fields as its flags name them. */
    fn run(
        &mut self,
        bytes: &[u8],
        body: &[u8],
        fields: &TrackFragment,
        track: &Track,
        moof_start: usize,
    ) -> Result<()> {
        let (version, flags, payload) = full_box(body)?;
        let mut cursor = Cursor::new(payload, "trun");
        let count = cursor.u32()?;
        if count > SAMPLE_LIMIT {
            return Err(Error::Malformed("trun sample count"));
        }
        if flags & TRUN_DATA_OFFSET != 0 {
            let offset = i64::from(cursor.i32()?);
            let base = fields.base_or(moof_start) as i64;
            self.data_pos =
                usize::try_from(base + offset).map_err(|_| Error::Malformed("data offset"))?;
        }
        let first_flags = if flags & TRUN_FIRST_FLAGS != 0 {
            Some(cursor.u32()?)
        } else {
            None
        };
        for index in 0..count {
            let raw = read_sample(
                &mut cursor,
                flags,
                version,
                index,
                first_flags,
                fields,
                track,
            )?;
            self.push(bytes, raw)?;
        }
        Ok(())
    }
    /** Place one sample at the data position, advance the clocks, and keep it. */
    fn push(&mut self, bytes: &[u8], raw: RawSample) -> Result<()> {
        if self.samples.len() >= self.room {
            return Err(Error::Malformed("sample count"));
        }
        if raw.size == 0 {
            return Err(Error::Malformed("zero-size sample"));
        }
        let end = self
            .data_pos
            .checked_add(raw.size)
            .ok_or(Error::Truncated("sample data"))?;
        if end > bytes.len() {
            return Err(Error::Truncated("sample data"));
        }
        self.samples.push(Sample {
            track: raw.track,
            offset: self.data_pos,
            size: raw.size,
            decode_time: self.decode_time,
            pts: self.decode_time as i64 + raw.composition - raw.edit_offset,
            duration: raw.duration,
            sync: raw.flags & SAMPLE_NON_SYNC == 0,
        });
        self.data_pos = end;
        self.end = end;
        self.decode_time = self.decode_time.wrapping_add(u64::from(raw.duration));
        Ok(())
    }
}

/** One sample as its trun fields give it, before it is placed in the segment. */
struct RawSample {
    track: u32,
    size: usize,
    duration: u32,
    flags: u32,
    composition: i64,
    edit_offset: i64,
}

/** Read one sample's fields in trun order, falling back from sample to first-sample to tfhd to trex. */
fn read_sample(
    cursor: &mut Cursor<'_>,
    flags: u32,
    version: u8,
    index: u32,
    first_flags: Option<u32>,
    fields: &TrackFragment,
    track: &Track,
) -> Result<RawSample> {
    let duration = if flags & TRUN_DURATION != 0 {
        cursor.u32()?
    } else {
        fields.default_duration.unwrap_or(track.default_duration)
    };
    let size = if flags & TRUN_SIZE != 0 {
        cursor.u32()?
    } else {
        fields.default_size.unwrap_or(track.default_size)
    };
    let fallback = fields.default_flags.unwrap_or(track.default_flags);
    let sample_flags = if flags & TRUN_FLAGS != 0 {
        cursor.u32()?
    } else if index == 0 {
        first_flags.unwrap_or(fallback)
    } else {
        fallback
    };
    let composition = if flags & TRUN_COMPOSITION == 0 {
        0
    } else if version == VERSION_LARGE {
        i64::from(cursor.i32()?)
    } else {
        i64::from(cursor.u32()?)
    };
    Ok(RawSample {
        track: track.id,
        size: size as usize,
        duration,
        flags: sample_flags,
        composition,
        edit_offset: track.edit_offset,
    })
}
