/**
 * What a Media Source element streams, kept on the native side: for each source buffer, its tracks and the coded
 * frames the script accepted, per track in decode order with their presentation times. The script owns the
 * buffered ranges and the MSE algorithm; this module only parses what it was handed (mars-mp4), clips frames to the
 * append window, removes ranges, and evicts frames behind the playhead at a keyframe, so what remains still decodes.
 */
use mars_mp4::{Error, Track, TrackKind, parse_init, parse_segment};
use std::collections::{BTreeMap, VecDeque};

/// How much media stays behind the playhead before eviction takes it, in seconds.
const KEEP_BEHIND: f64 = 30.0;
/// The missing-initialization error's box name: a media segment needs the moov that came before it.
const MOOV: [u8; 4] = *b"moov";

/** One coded frame: where it decodes, where it presents, how long it lasts, whether it is a sync point, and its bytes. */
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /** Decode time in seconds, shifted by the buffer's timestamp offset. */
    pub decode: f64,
    /** Presentation time in seconds, shifted by the buffer's timestamp offset. */
    pub pts: f64,
    /** Presentation duration in seconds. */
    pub duration: f64,
    /** Whether a decoder can start here without the frames before it. */
    pub sync: bool,
    /** The frame's bytes as the segment carried them (an AVCC or raw AAC access unit). */
    pub data: Vec<u8>,
}

/** One track of a source buffer and the frames it has accepted, in decode order. */
#[derive(Clone, Debug, PartialEq)]
pub struct Lane {
    /** The track's description from its init segment: codec, decoder configuration, layout. */
    pub track: Track,
    /** The accepted frames, oldest decode time first. */
    pub frames: VecDeque<Frame>,
}

impl Lane {
    /** A lane for a track with no frames yet. */
    fn new(track: Track) -> Self {
        Self {
            track,
            frames: VecDeque::new(),
        }
    }
    /** The track's kind, which decides whether its frames are pictures or sound. */
    pub fn kind(&self) -> TrackKind {
        self.track.kind
    }
}

/** One source buffer: the tracks its init segment declared, and a lane per track. */
#[derive(Debug, Default)]
struct Buffer {
    tracks: Vec<Track>,
    lanes: BTreeMap<u32, Lane>,
}

/** One append the script accepted: its whole boxes, how many lead an init segment, and the buffer's offset and window. */
#[derive(Clone, Copy, Debug)]
pub struct Append<'a> {
    /** Whole top-level boxes: the init segment (if `init_len` is nonzero) followed by media segments. */
    pub bytes: &'a [u8],
    /** Bytes at the front of `bytes` that are the init segment. */
    pub init_len: usize,
    /** The buffer's timestamp offset, in seconds. */
    pub offset: f64,
    /** The append window's start and optional end, in seconds. */
    pub window: (f64, Option<f64>),
}

/** Every source buffer of one Media Source player. Main thread only, like the players that own it. */
#[derive(Debug, Default)]
pub struct Stream {
    buffers: BTreeMap<u32, Buffer>,
}

impl Stream {
    /** Apply one accepted append to a buffer. A media segment with no init segment before it is refused. */
    pub fn append(&mut self, buffer: u32, append: Append<'_>) -> Result<(), Error> {
        let (init, media) = append
            .bytes
            .split_at(append.init_len.min(append.bytes.len()));
        let target = self.buffers.entry(buffer).or_default();
        if !init.is_empty() {
            target.adopt(parse_init(init)?);
        }
        if media.is_empty() {
            return Ok(());
        }
        if target.tracks.is_empty() {
            return Err(Error::Missing(MOOV));
        }
        let samples = parse_segment(media, &target.tracks)?;
        for sample in samples {
            let Some(lane) = target.lanes.get_mut(&sample.track) else {
                continue;
            };
            let scale = lane.track.timescale as f64;
            let pts = sample.pts as f64 / scale + append.offset;
            let duration = f64::from(sample.duration) / scale;
            if clipped(pts, pts + duration, append.window).is_none() {
                continue;
            }
            lane.frames.push_back(Frame {
                decode: sample.decode_time as f64 / scale + append.offset,
                pts,
                duration,
                sync: sample.sync,
                data: media[sample.offset..sample.offset + sample.size].to_vec(),
            });
        }
        Ok(())
    }

    /** `remove`: drop the frames of one buffer whose presentation time lies in `[start, end)`. */
    pub fn remove(&mut self, buffer: u32, start: f64, end: f64) {
        let Some(target) = self.buffers.get_mut(&buffer) else {
            return;
        };
        for lane in target.lanes.values_mut() {
            lane.frames
                .retain(|frame| frame.pts < start || frame.pts >= end);
        }
    }

    /**
     * Eviction behind the playhead: in each lane, drop everything before the first sync frame at or after the point
     * `KEEP_BEHIND` seconds behind the playhead. Cutting at a sync frame keeps the rest decodable; a lane with no such
     * frame keeps everything, since nothing behind it is old enough to go.
     */
    pub fn evict_behind(&mut self, playhead: f64) {
        let cut = playhead - KEEP_BEHIND;
        for lane in self.lanes_mut() {
            let keep_from = lane
                .frames
                .iter()
                .position(|frame| frame.sync && frame.pts >= cut)
                .unwrap_or(0);
            lane.frames.drain(..keep_from);
        }
    }

    /** Every lane of every buffer, for the presenters that feed decoders and renderers. */
    pub fn lanes(&self) -> impl Iterator<Item = &Lane> {
        self.buffers
            .values()
            .flat_map(|buffer| buffer.lanes.values())
    }

    /** Every lane of every buffer, mutably, for eviction. */
    fn lanes_mut(&mut self) -> impl Iterator<Item = &mut Lane> {
        self.buffers
            .values_mut()
            .flat_map(|buffer| buffer.lanes.values_mut())
    }
}

impl Buffer {
    /** Take an init segment's tracks: each track keeps its frames if it was declared before, and a new one gets a lane. */
    fn adopt(&mut self, tracks: Vec<Track>) {
        for track in &tracks {
            self.lanes
                .entry(track.id)
                .and_modify(|lane| lane.track = track.clone())
                .or_insert_with(|| Lane::new(track.clone()));
        }
        self.tracks = tracks;
    }
}

/** The part of `[start, end)` inside the append window, or None when none of it is. */
fn clipped(start: f64, end: f64, window: (f64, Option<f64>)) -> Option<(f64, f64)> {
    let (window_start, window_end) = window;
    let start = start.max(window_start);
    let end = end.min(window_end.unwrap_or(f64::INFINITY));
    (end > start).then_some((start, end))
}

#[cfg(test)]
mod tests {
    /**
     * Synthetic segments for the stream tests: one video track at timescale 1000 with an avcC record, and media
     * segments whose samples take their sizes and sync flags from the caller.
     */
    use super::{Append, Stream};

    /** A box of `kind` around `payload`. */
    fn bx(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        [
            &((payload.len() + 8) as u32).to_be_bytes()[..],
            kind,
            payload,
        ]
        .concat()
    }

    /** A version-0 full box: the version byte, 24-bit flags, then the payload. */
    fn fb(kind: &[u8; 4], flags: u32, payload: &[u8]) -> Vec<u8> {
        bx(
            kind,
            &[&[0u8][..], &flags.to_be_bytes()[1..], payload].concat(),
        )
    }

    /** A moov with one video track: id 1, timescale 1000, 320 by 240, and an avc1 entry with a short avcC. */
    fn init() -> Vec<u8> {
        let mut tkhd = vec![0u8; 8];
        tkhd.extend(1u32.to_be_bytes());
        tkhd.extend([0u8; 68]);
        tkhd.extend((320u32 << 16).to_be_bytes());
        tkhd.extend((240u32 << 16).to_be_bytes());
        let mut mdhd = vec![0u8; 8];
        mdhd.extend(1000u32.to_be_bytes());
        mdhd.extend([0u8; 4]);
        let hdlr = [&[0u8; 4][..], b"vide", &[0u8; 13][..]].concat();
        let avcc = bx(b"avcC", &[1, 0x42, 0xC0, 0x1E, 0xFF, 0xE1, 0, 0]);
        let avc1 = bx(b"avc1", &[&[0u8; 78][..], &avcc].concat());
        let stsd = fb(b"stsd", 0, &[1u32.to_be_bytes().as_slice(), &avc1].concat());
        let stbl = bx(b"stbl", &stsd);
        let minf = bx(b"minf", &stbl);
        let mdia = bx(
            b"mdia",
            &[fb(b"mdhd", 0, &mdhd), fb(b"hdlr", 0, &hdlr), minf].concat(),
        );
        let trak = bx(b"trak", &[fb(b"tkhd", 0, &tkhd), mdia].concat());
        bx(b"moov", &trak)
    }

    /** A media segment: one moof with a single traf, then the mdat holding `data`. Each sample is (duration, size, sync). */
    fn media(decode: u32, samples: &[(u32, u32, bool)], data: &[u8]) -> Vec<u8> {
        let mut rows = Vec::new();
        for &(duration, size, sync) in samples {
            let flags = if sync { 0 } else { 0x0001_0000 };
            rows.extend(
                [duration, size, flags]
                    .iter()
                    .flat_map(|v: &u32| v.to_be_bytes()),
            );
        }
        let count = samples.len() as u32;
        let trun = |offset: u32| {
            let payload = [&count.to_be_bytes()[..], &offset.to_be_bytes()[..], &rows].concat();
            fb(b"trun", 0x701, &payload)
        };
        let traf = |offset: u32| {
            let tfhd = fb(b"tfhd", 0x0002_0000, &1u32.to_be_bytes());
            let tfdt = fb(b"tfdt", 0, &decode.to_be_bytes());
            bx(b"traf", &[tfhd, tfdt, trun(offset)].concat())
        };
        let placeholder = bx(b"moof", &traf(0));
        let moof = bx(b"moof", &traf(placeholder.len() as u32 + 8));
        [moof, bx(b"mdat", data)].concat()
    }

    /** The first append of a buffer: the init segment and a media segment together, at offset zero with no window. */
    fn first(bytes: &[u8], init_len: usize) -> Append<'_> {
        Append {
            bytes,
            init_len,
            offset: 0.0,
            window: (0.0, None),
        }
    }

    /** A media segment appended with a timestamp offset starts its frames at the offset, not at the segment's own time. */
    #[test]
    fn media_frames_take_the_buffer_timestamp_offset() {
        let init = init();
        let segment = media(0, &[(1000, 2, true)], &[7, 8]);
        let mut stream = Stream::default();
        let bytes = [init.clone(), segment.clone()].concat();
        stream.append(1, first(&bytes, init.len())).unwrap();
        let shifted = Append {
            bytes: &segment,
            init_len: 0,
            offset: 10.0,
            window: (0.0, None),
        };
        stream.append(1, shifted).unwrap();
        let frames: Vec<f64> = stream
            .lanes()
            .next()
            .unwrap()
            .frames
            .iter()
            .map(|f| f.pts)
            .collect();
        assert_eq!(frames, vec![0.0, 10.0]);
    }

    /** An init segment gives each track a lane with its codec, decoder configuration and display size. */
    #[test]
    fn init_segment_gives_each_track_a_lane_with_its_config() {
        let init = init();
        let mut stream = Stream::default();
        stream.append(1, first(&init, init.len())).unwrap();
        let lane = stream.lanes().next().unwrap();
        assert_eq!(lane.track.codec, "avc1.42C01E");
        assert_eq!(
            lane.track.config,
            vec![1, 0x42, 0xC0, 0x1E, 0xFF, 0xE1, 0, 0]
        );
        assert_eq!((lane.track.width, lane.track.height), (320, 240));
        assert!(lane.frames.is_empty());
    }

    /** A media segment before any init segment is refused, and nothing is buffered. */
    #[test]
    fn a_media_segment_without_an_init_segment_is_refused() {
        let segment = media(0, &[(1000, 1, true)], &[1]);
        let mut stream = Stream::default();
        assert!(stream.append(1, first(&segment, 0)).is_err());
        assert_eq!(stream.lanes().count(), 0);
    }

    /** A frame that ends before the append window starts is dropped; a frame the window only partly covers is kept whole. */
    #[test]
    fn frames_outside_the_append_window_are_dropped() {
        let init = init();
        let segment = media(0, &[(1000, 1, true), (1000, 1, true)], &[1, 2]);
        let bytes = [init.clone(), segment].concat();
        let mut stream = Stream::default();
        stream
            .append(
                1,
                Append {
                    bytes: &bytes,
                    init_len: init.len(),
                    offset: 0.0,
                    window: (1.5, None),
                },
            )
            .unwrap();
        let frames = &stream.lanes().next().unwrap().frames;
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].pts, 1.0);
        assert_eq!(frames[0].data, vec![2]);
    }

    /** remove drops the frames whose presentation starts inside its bounds and keeps the rest. */
    #[test]
    fn remove_drops_frames_between_its_bounds() {
        let init = init();
        let segment = media(
            0,
            &[(1000, 1, true), (1000, 1, true), (1000, 1, true)],
            &[1, 2, 3],
        );
        let bytes = [init.clone(), segment].concat();
        let mut stream = Stream::default();
        stream.append(1, first(&bytes, init.len())).unwrap();
        stream.remove(1, 1.0, 2.0);
        let pts: Vec<f64> = stream
            .lanes()
            .next()
            .unwrap()
            .frames
            .iter()
            .map(|f| f.pts)
            .collect();
        assert_eq!(pts, vec![0.0, 2.0]);
    }

    /** Eviction cuts only at a sync frame, so the frames it keeps still decode: at playhead 50 the cut is 20, the sync frame there. */
    #[test]
    fn eviction_keeps_a_decodable_run_behind_the_playhead() {
        let init = init();
        let samples: Vec<(u32, u32, bool)> = (0..60).map(|i| (1000, 1, i % 20 == 0)).collect();
        let data = vec![0u8; 60];
        let segment = media(0, &samples, &data);
        let bytes = [init.clone(), segment].concat();
        let mut stream = Stream::default();
        stream.append(1, first(&bytes, init.len())).unwrap();
        stream.evict_behind(50.0);
        let frames = &stream.lanes().next().unwrap().frames;
        assert_eq!(frames.front().unwrap().pts, 20.0);
        assert!(frames.front().unwrap().sync);
        assert_eq!(frames.len(), 40);
    }

    /** A box whose size runs past its segment is refused, and nothing is buffered. */
    #[test]
    fn a_refused_segment_leaves_the_buffer_empty() {
        let init = init();
        let hostile = [
            init.clone(),
            vec![0xFF, 0xFF, 0xFF, 0xFF, b'm', b'o', b'o', b'f', 1, 2, 3],
        ]
        .concat();
        let mut stream = Stream::default();
        let outcome = stream.append(1, first(&hostile, init.len()));
        assert!(outcome.is_err());
        assert_eq!(
            stream.lanes().flat_map(|lane| lane.frames.iter()).count(),
            0
        );
    }
}
