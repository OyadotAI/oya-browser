#![allow(dead_code)]
/** Test helpers that build fragmented MP4 bytes by hand: boxes, init segments and media segments with a chosen layout. */
/** One plain box: size, type and payload. */
pub fn bx(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    out
}

/** One full box: a box whose payload starts with version and 24-bit flags. */
pub fn fb(kind: &[u8; 4], version: u8, flags: u32, payload: &[u8]) -> Vec<u8> {
    let mut body = vec![version];
    body.extend_from_slice(&flags.to_be_bytes()[1..]);
    body.extend_from_slice(payload);
    bx(kind, &body)
}

/** A 32-bit big-endian field, as bytes. */
pub fn u32s(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_be_bytes())
        .collect()
}

/** The parameters of one track in the test init segment. */
#[derive(Clone)]
pub struct TrackSpec {
    pub id: u32,
    pub timescale: u32,
    pub audio: bool,
    /** Default duration, size and flags in trex. */
    pub trex: [u32; 3],
    /** An elst with one entry of this media time, when set. */
    pub media_time: Option<i32>,
    /** The tkhd display size in pixels (zero for audio). */
    pub size: (u32, u32),
}

impl TrackSpec {
    /** A video track 1 at timescale 1000 with zero trex defaults and no edit list. */
    pub fn video(id: u32) -> Self {
        Self {
            id,
            timescale: 1000,
            audio: false,
            trex: [0, 0, 0],
            media_time: None,
            size: (320, 240),
        }
    }
    /** An audio track at timescale 48000. */
    pub fn audio(id: u32) -> Self {
        Self {
            id,
            timescale: 48000,
            audio: true,
            trex: [0, 0, 0],
            media_time: None,
            size: (0, 0),
        }
    }
}

/** The avc1 sample entry: fixed visual fields, then an avcC record (profile 100, compatibility 0, level 31). */
fn avc1_entry() -> Vec<u8> {
    let mut body = vec![0u8; 78];
    body.extend(bx(b"avcC", &[1, 0x64, 0x00, 0x1f, 0xff, 0xe1, 0, 0]));
    bx(b"avc1", &body)
}

/** The mp4a sample entry: fixed audio fields, then an esds with an AAC-LC decoder specific info. */
fn mp4a_entry() -> Vec<u8> {
    let specific = [0x05, 0x02, 0x11, 0x90];
    let config = [
        vec![0x40, 0x15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        specific.to_vec(),
    ]
    .concat();
    let es = [vec![0, 1, 0], vec![0x04, config.len() as u8], config].concat();
    let esds = [vec![0x03, es.len() as u8], es].concat();
    let mut body = vec![0u8; 28];
    body.extend(fb(b"esds", 0, 0, &esds));
    bx(b"mp4a", &body)
}

/** An ftyp, then a moov with one trak per spec and an mvex with a trex for each. */
pub fn init_segment(tracks: &[TrackSpec]) -> Vec<u8> {
    let traks: Vec<Vec<u8>> = tracks.iter().map(trak).collect();
    let trexes: Vec<Vec<u8>> = tracks.iter().map(trex).collect();
    let moov = bx(
        b"moov",
        &[traks.concat(), bx(b"mvex", &trexes.concat())].concat(),
    );
    [bx(b"ftyp", b"iso5\0\0\0\0mp41"), moov].concat()
}

/** One trak: tkhd, edts (when asked), and mdia with the timescale, handler and sample entry. */
fn trak(spec: &TrackSpec) -> Vec<u8> {
    let tkhd = fb(
        b"tkhd",
        0,
        0,
        &[
            u32s(&[0, 0, spec.id, 0]),
            vec![0u8; 52],
            u32s(&[spec.size.0 << 16, spec.size.1 << 16]),
        ]
        .concat(),
    );
    let mdhd = fb(b"mdhd", 0, 0, &u32s(&[0, 0, spec.timescale, 0]));
    let (handler, entry) = if spec.audio {
        (*b"soun", mp4a_entry())
    } else {
        (*b"vide", avc1_entry())
    };
    let hdlr = fb(
        b"hdlr",
        0,
        0,
        &[u32s(&[0]), handler.to_vec(), vec![0u8; 13]].concat(),
    );
    let stsd = fb(b"stsd", 0, 0, &[u32s(&[1]), entry].concat());
    let stbl = bx(b"stbl", &stsd);
    let mdia = bx(b"mdia", &[mdhd, hdlr, bx(b"minf", &stbl)].concat());
    let edts = match spec.media_time {
        Some(time) => bx(
            b"edts",
            &fb(b"elst", 0, 0, &u32s(&[1, 0, time as u32, 0x0001_0000])),
        ),
        None => Vec::new(),
    };
    bx(b"trak", &[tkhd, edts, bx(b"mdia", &mdia[8..])].concat())
}

/** One trex: the track's default duration, size and flags. */
fn trex(spec: &TrackSpec) -> Vec<u8> {
    let [duration, size, flags] = spec.trex;
    fb(b"trex", 0, 0, &u32s(&[spec.id, 1, duration, size, flags]))
}

/** A tfhd for a track with the given flags and fields, in the order the flags name. */
pub fn tfhd(track: u32, flags: u32, fields: &[u32]) -> Vec<u8> {
    fb(b"tfhd", 0, flags, &u32s(&[&[track][..], fields].concat()))
}

/** A trun with the given flags, data offset and per-sample rows (duration, size, flags, composition). */
pub fn trun(
    version: u8,
    flags: u32,
    data_offset: i32,
    first_flags: Option<u32>,
    rows: &[[u32; 4]],
) -> Vec<u8> {
    let mut payload = u32s(&[rows.len() as u32]);
    if flags & 0x1 != 0 {
        payload.extend_from_slice(&data_offset.to_be_bytes());
    }
    if let Some(first) = first_flags {
        payload.extend(u32s(&[first]));
    }
    for row in rows {
        for (bit, value) in [
            (0x100, row[0]),
            (0x200, row[1]),
            (0x400, row[2]),
            (0x800, row[3]),
        ] {
            if flags & bit != 0 {
                payload.extend(u32s(&[value]));
            }
        }
    }
    fb(b"trun", version, flags, &payload)
}

/** A media segment: one moof holding the given traf bodies, then an mdat holding `data`. The traf bodies are built
 *  by `traf`, which receives the byte offset of the mdat payload relative to the moof start. */
pub fn media_segment(data: &[u8], traf: impl Fn(i32) -> Vec<Vec<u8>>) -> Vec<u8> {
    let placeholder = moof(&traf(0));
    let data_offset = (placeholder.len() + 8) as i32;
    let moof = moof(&traf(data_offset));
    [moof, bx(b"mdat", data)].concat()
}

/** A moof holding the given traf bodies, each boxed as a traf. */
fn moof(trafs: &[Vec<u8>]) -> Vec<u8> {
    let mfhd = fb(b"mfhd", 0, 0, &u32s(&[1]));
    let bodies: Vec<Vec<u8>> = trafs.iter().map(|body| bx(b"traf", body)).collect();
    bx(b"moof", &[mfhd, bodies.concat()].concat())
}

/** A tfdt, version 0 or 1, with the given base decode time. */
pub fn tfdt(version: u8, time: u64) -> Vec<u8> {
    if version == 1 {
        fb(b"tfdt", 1, 0, &time.to_be_bytes())
    } else {
        fb(b"tfdt", 0, 0, &(time as u32).to_be_bytes())
    }
}
