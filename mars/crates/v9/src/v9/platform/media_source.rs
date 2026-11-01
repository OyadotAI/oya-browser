/**
 * Media Source's coded frame processing for one append. The bytes are read as whole ISO BMFF boxes with
 * mars-mp4: an init segment gives the tracks, a media segment gives the samples, and each sample's
 * presentation interval, shifted by the buffer's timestamp offset and clipped to its append window, joins
 * the buffered ranges. The script keeps the SourceBuffer's state (its init bytes, offset, window and ranges)
 * and asks this module what each append adds, so the MSE algorithm stays in script and the parsing and
 * interval arithmetic stay in Rust, where they are tested.
 */
use mars_mp4::{
    BoxSpan, Error as Mp4Error, Track, TrackKind, complete_boxes, parse_init, parse_segment,
};
use serde::{Deserialize, Serialize};

/** The four-character type of a movie box, which carries the tracks an init segment declares. */
const MOOV: [u8; 4] = *b"moov";
/** The four-character type of a movie fragment, the first box of every media segment. */
const MOOF: [u8; 4] = *b"moof";

/** What the script knows about the SourceBuffer an append goes to, as it sends it. */
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    /** The buffer's timestamp offset, in seconds, added to every sample's presentation time. */
    pub offset: f64,
    /** The append window's start, in seconds; samples that end before it are dropped. */
    pub window_start: f64,
    /** The append window's end, in seconds; `None` (JSON null) means no end. */
    pub window_end: Option<f64>,
    /** The buffered ranges before this append, sorted and disjoint, as `[start, end]` pairs. */
    pub ranges: Vec<[f64; 2]>,
}

/** One track an init segment declares, as far as the script needs to check and describe it. */
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    /** `video`, `audio` or `other`. */
    pub kind: &'static str,
    /** The codec string, such as `avc1.64001F` or `mp4a.40.2`. */
    pub codec: String,
    /** The display width in pixels; zero without a picture. */
    pub width: u32,
    /** The display height in pixels; zero without a picture. */
    pub height: u32,
}

/** What one append did: how many bytes it consumed, its init segment if it carried one, and the new ranges. */
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /** Bytes of the append that formed whole boxes; the rest waits for the next append. */
    pub consumed: usize,
    /** Bytes at the front of the append that are its init segment (ftyp and moov), or zero. */
    pub init_len: usize,
    /** The tracks of the init segment this append carried; empty when it carried none. */
    pub tracks: Vec<TrackInfo>,
    /** The buffered ranges after this append, sorted and merged. */
    pub ranges: Vec<[f64; 2]>,
}

/** Why an append was refused: the script raises an append error (decode) when it gets one of these. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /** A media segment arrived before any init segment described its tracks. */
    NoInitSegment,
    /** The bytes are not valid ISO BMFF, or a box they need is missing. */
    Container(Mp4Error),
}

impl Refusal {
    /** A one-line description for the page's error event. */
    pub fn message(&self) -> String {
        match self {
            Self::NoInitSegment => "media segment appended before an initialization segment".into(),
            Self::Container(error) => format!("invalid MP4 data: {error}"),
        }
    }
}

impl From<Mp4Error> for Refusal {
    /** Any parse failure is a container refusal. */
    fn from(error: Mp4Error) -> Self {
        Self::Container(error)
    }
}

/**
 * Process one append. `stored_init` is the init segment bytes the buffer kept from an earlier append; an init
 * segment in this append replaces it (the caller stores `bytes[..init_len]` when `init_len` is nonzero).
 * An append with no whole box yet consumes nothing and is not an error.
 */
pub fn append(
    stored_init: Option<&[u8]>,
    bytes: &[u8],
    request: &Request,
) -> Result<Outcome, Refusal> {
    let spans = complete_boxes(bytes)?;
    let consumed = spans.last().map_or(0, |span| span.end);
    let init_len = init_length(&spans);
    let has_media = spans.iter().any(|span| span.kind == MOOF);
    if init_len == 0 && has_media && stored_init.is_none() {
        return Err(Refusal::NoInitSegment);
    }
    let init = if init_len > 0 {
        Some(&bytes[..init_len])
    } else {
        stored_init
    };
    let Some(init) = init else {
        return Ok(untouched(consumed, request));
    };
    let tracks = parse_init(init)?;
    let samples = parse_segment(&bytes[..consumed], &tracks)?;
    let intervals = sample_intervals(&samples, &tracks, request)?;
    Ok(Outcome {
        consumed,
        init_len,
        tracks: if init_len > 0 {
            describe(&tracks)
        } else {
            Vec::new()
        },
        ranges: merge(request.ranges.iter().copied().chain(intervals).collect()),
    })
}

/** An outcome for an append with nothing to process yet: the ranges stay as they were. */
fn untouched(consumed: usize, request: &Request) -> Outcome {
    Outcome {
        consumed,
        init_len: 0,
        tracks: Vec::new(),
        ranges: request.ranges.clone(),
    }
}

/**
 * The bytes an init segment takes: everything up to the end of its last box when a moov is among the boxes
 * before the first moof, else zero. A moov that is not yet whole is not counted, so it waits for more bytes.
 */
fn init_length(spans: &[BoxSpan]) -> usize {
    let before_media: Vec<&BoxSpan> = spans.iter().take_while(|span| span.kind != MOOF).collect();
    let has_moov = before_media.iter().any(|span| span.kind == MOOV);
    match before_media.last() {
        Some(last) if has_moov => last.end,
        _ => 0,
    }
}

/** The presentation interval of each sample, in seconds, clipped to the append window. */
fn sample_intervals(
    samples: &[mars_mp4::Sample],
    tracks: &[Track],
    request: &Request,
) -> Result<Vec<[f64; 2]>, Refusal> {
    let mut intervals = Vec::with_capacity(samples.len());
    for sample in samples {
        let track = tracks
            .iter()
            .find(|track| track.id == sample.track)
            .ok_or(Mp4Error::Missing(*b"trak"))?;
        if track.timescale == 0 {
            return Err(Mp4Error::Malformed("timescale").into());
        }
        let scale = f64::from(track.timescale);
        let start = sample.pts as f64 / scale + request.offset;
        let end = start + f64::from(sample.duration) / scale;
        if let Some(clipped) = clip(start, end, request) {
            intervals.push(clipped);
        }
    }
    Ok(intervals)
}

/** The part of `[start, end)` inside the append window, or None when none of it is. */
fn clip(start: f64, end: f64, request: &Request) -> Option<[f64; 2]> {
    let window_end = request.window_end.unwrap_or(f64::INFINITY);
    let (start, end) = (start.max(request.window_start), end.min(window_end));
    (end > start).then_some([start, end])
}

/** Sort intervals and join the ones that touch or overlap; the result is sorted and disjoint. */
fn merge(mut intervals: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    intervals.sort_by(|a, b| a[0].total_cmp(&b[0]));
    let mut merged: Vec<[f64; 2]> = Vec::with_capacity(intervals.len());
    for interval in intervals {
        match merged.last_mut() {
            Some(last) if interval[0] <= last[1] => last[1] = last[1].max(interval[1]),
            _ => merged.push(interval),
        }
    }
    merged
}

/** The script-facing description of each init track. */
fn describe(tracks: &[Track]) -> Vec<TrackInfo> {
    tracks
        .iter()
        .map(|track| TrackInfo {
            kind: match track.kind {
                TrackKind::Video => "video",
                TrackKind::Audio => "audio",
                TrackKind::Other => "other",
            },
            codec: track.codec.clone(),
            width: track.width,
            height: track.height,
        })
        .collect()
}
