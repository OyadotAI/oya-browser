/** Media segments through the public surface: sample placement, times, sync flags, defaults, bases and refusals. */
mod support;

use mars_mp4::{Error, Sample, parse_init, parse_segment};
use support::{TrackSpec, bx, fb, init_segment, media_segment, tfdt, tfhd, trun, u32s};

/** Flags a trun or tfhd sets for each field it carries. */
const DATA_OFFSET: u32 = 0x1;
const DURATION: u32 = 0x100;
const SIZE: u32 = 0x200;
const FLAGS: u32 = 0x400;
const COMPOSITION: u32 = 0x800;
const FIRST_FLAGS: u32 = 0x4;
const TFHD_BASE: u32 = 0x1;
const TFHD_DEFAULT_SIZE: u32 = 0x10;
const TFHD_BASE_IS_MOOF: u32 = 0x20000;
/** A sample flag marking a non-sync sample. */
const NON_SYNC: u32 = 0x0001_0000;

/** The tracks of an init segment with one video track 1 whose edit offset is `media_time`. */
fn video_tracks(media_time: Option<i32>) -> Vec<mars_mp4::Track> {
    let spec = TrackSpec {
        media_time,
        ..TrackSpec::video(1)
    };
    parse_init(&init_segment(&[spec])).unwrap()
}

/** Samples of a single traf with a tfhd, tfdt and one trun, the mdat carrying `data`. */
fn one_run(data: &[u8], flags: u32, rows: &[[u32; 4]], version: u8) -> Vec<Sample> {
    let bytes = media_segment(data, |offset| {
        vec![
            [
                tfhd(1, 0, &[]),
                tfdt(0, 100),
                trun(version, flags | DATA_OFFSET, offset, None, rows),
            ]
            .concat(),
        ]
    });
    parse_segment(&bytes, &video_tracks(None)).unwrap()
}

/** Each sample's bytes sit where its trun's sizes place them, one after another from the data offset. */
#[test]
fn segment_places_samples_one_after_another_from_the_data_offset() {
    let samples = one_run(
        &[7; 30],
        DURATION | SIZE,
        &[[10, 10, 0, 0], [10, 20, 0, 0]],
        0,
    );
    let placed: Vec<(usize, usize)> = samples.iter().map(|s| (s.offset, s.size)).collect();
    let moof_len = placed[0].0;
    assert_eq!(placed, vec![(moof_len, 10), (moof_len + 10, 20)]);
}

/** Decode time starts at tfdt and advances by each sample's duration. */
#[test]
fn segment_advances_decode_time_by_each_duration() {
    let samples = one_run(
        &[7; 20],
        DURATION | SIZE,
        &[[40, 10, 0, 0], [60, 10, 0, 0]],
        0,
    );
    let times: Vec<u64> = samples.iter().map(|s| s.decode_time).collect();
    assert_eq!(times, vec![100, 140]);
}

/** Presentation time is decode time plus the composition offset, minus the track's edit offset. */
#[test]
fn segment_presents_at_decode_time_plus_composition_minus_edit() {
    let bytes = media_segment(&[7; 10], |offset| {
        vec![
            [
                tfhd(1, 0, &[]),
                tfdt(0, 500),
                trun(
                    0,
                    COMPOSITION | SIZE | DATA_OFFSET,
                    offset,
                    None,
                    &[[0, 10, 0, 300]],
                ),
            ]
            .concat(),
        ]
    });
    let samples = parse_segment(&bytes, &video_tracks(Some(1000))).unwrap();
    assert_eq!(samples[0].pts, 500 + 300 - 1000);
}

/** A version 1 trun reads its composition offset as signed, so a negative one moves presentation earlier. */
#[test]
fn segment_reads_a_negative_composition_offset_in_version_one() {
    let negative = (-40i32) as u32;
    let bytes = media_segment(&[7; 10], |offset| {
        vec![
            [
                tfhd(1, 0, &[]),
                tfdt(0, 500),
                trun(
                    1,
                    COMPOSITION | SIZE | DATA_OFFSET,
                    offset,
                    None,
                    &[[0, 10, 0, negative]],
                ),
            ]
            .concat(),
        ]
    });
    let samples = parse_segment(&bytes, &video_tracks(None)).unwrap();
    assert_eq!(samples[0].pts, 460);
}

/** A sample without the non-sync bit is a sync point; one with it is not. */
#[test]
fn segment_marks_sync_from_the_non_sync_sample_flag() {
    let samples = one_run(
        &[7; 20],
        DURATION | SIZE | FLAGS,
        &[[10, 10, 0, 0], [10, 10, NON_SYNC, 0]],
        0,
    );
    let sync: Vec<bool> = samples.iter().map(|s| s.sync).collect();
    assert_eq!(sync, vec![true, false]);
}

/** The first sample's flags may differ from the rest, through first-sample-flags. */
#[test]
fn segment_takes_first_sample_flags_for_the_first_sample_only() {
    let bytes = media_segment(&[7; 20], |offset| {
        vec![
            [
                tfhd(1, 0, &[]),
                tfdt(0, 0),
                trun(
                    0,
                    SIZE | DATA_OFFSET | FIRST_FLAGS,
                    offset,
                    Some(NON_SYNC),
                    &[[0, 10, 0, 0], [0, 10, 0, 0]],
                ),
            ]
            .concat(),
        ]
    });
    let samples = parse_segment(&bytes, &video_tracks(None)).unwrap();
    assert_eq!(
        samples.iter().map(|s| s.sync).collect::<Vec<_>>(),
        vec![false, true]
    );
}

/** Missing sample sizes come from the tfhd default, and missing durations from the trex default. */
#[test]
fn segment_falls_back_to_tfhd_then_trex_defaults() {
    let bytes = media_segment(&[7; 20], |offset| {
        vec![
            [
                tfhd(1, TFHD_DEFAULT_SIZE, &[10]),
                tfdt(0, 0),
                trun(0, DATA_OFFSET, offset, None, &[[0, 0, 0, 0], [0, 0, 0, 0]]),
            ]
            .concat(),
        ]
    });
    let spec = TrackSpec {
        trex: [33, 0, 0],
        ..TrackSpec::video(1)
    };
    let tracks = parse_init(&init_segment(&[spec])).unwrap();
    let samples = parse_segment(&bytes, &tracks).unwrap();
    assert_eq!(
        samples
            .iter()
            .map(|s| (s.size, s.duration))
            .collect::<Vec<_>>(),
        vec![(10, 33), (10, 33)]
    );
}

/** A second traf with no explicit base continues where the first one's data ends. */
#[test]
fn segment_continues_a_second_fragment_after_the_first_one_ends() {
    let bytes = media_segment(&[7; 20], |offset| {
        let first = [
            tfhd(1, 0, &[]),
            tfdt(0, 0),
            trun(0, SIZE | DATA_OFFSET, offset, None, &[[0, 10, 0, 0]]),
        ]
        .concat();
        let second = [
            tfhd(1, 0, &[]),
            tfdt(0, 10),
            trun(0, SIZE, 0, None, &[[0, 10, 0, 0]]),
        ]
        .concat();
        vec![first, second]
    });
    let samples = parse_segment(&bytes, &video_tracks(None)).unwrap();
    assert_eq!(samples[1].offset, samples[0].offset + 10);
}

/** An explicit tfhd base offset places the data, not the moof start. */
#[test]
fn segment_honours_an_explicit_base_data_offset() {
    let bytes = media_segment(&[7; 10], |_| {
        let base = fb(b"tfhd", 0, TFHD_BASE, &u32s(&[1, 0, 0]));
        vec![[base, tfdt(0, 0), trun(0, SIZE, 0, None, &[[0, 10, 0, 0]])].concat()]
    });
    let samples = parse_segment(&bytes, &video_tracks(None)).unwrap();
    assert_eq!(samples[0].offset, 0);
}

/** A fragment flagged base-is-moof starts at the moof even after an earlier fragment. */
#[test]
fn segment_resets_to_the_moof_when_the_base_is_moof() {
    let bytes = media_segment(&[7; 20], |offset| {
        let first = [
            tfhd(1, 0, &[]),
            tfdt(0, 0),
            trun(0, SIZE | DATA_OFFSET, offset, None, &[[0, 10, 0, 0]]),
        ]
        .concat();
        let second = [
            fb(b"tfhd", 0, TFHD_BASE_IS_MOOF, &u32s(&[1])),
            tfdt(0, 10),
            trun(0, SIZE | DATA_OFFSET, offset, None, &[[0, 10, 0, 0]]),
        ]
        .concat();
        vec![first, second]
    });
    let samples = parse_segment(&bytes, &video_tracks(None)).unwrap();
    assert_eq!(samples[1].offset, samples[0].offset);
}

/** A version 1 tfdt carries a 64-bit decode time. */
#[test]
fn segment_reads_a_64_bit_decode_time_from_tfdt_version_one() {
    let bytes = media_segment(&[7; 10], |offset| {
        vec![
            [
                tfhd(1, 0, &[]),
                tfdt(1, 1 << 40),
                trun(0, SIZE | DATA_OFFSET, offset, None, &[[0, 10, 0, 0]]),
            ]
            .concat(),
        ]
    });
    let samples = parse_segment(&bytes, &video_tracks(None)).unwrap();
    assert_eq!(samples[0].decode_time, 1 << 40);
}

/** A sample whose bytes run past the segment is refused, never sliced past its end. */
#[test]
fn segment_refuses_a_sample_beyond_the_segment() {
    let bytes = media_segment(&[7; 4], |offset| {
        vec![
            [
                tfhd(1, 0, &[]),
                tfdt(0, 0),
                trun(0, SIZE | DATA_OFFSET, offset, None, &[[0, 100, 0, 0]]),
            ]
            .concat(),
        ]
    });
    assert!(matches!(
        parse_segment(&bytes, &video_tracks(None)),
        Err(Error::Truncated(_))
    ));
}

/** A fragment for a track the init segment does not declare is refused. */
#[test]
fn segment_refuses_a_fragment_for_an_unknown_track() {
    let bytes = media_segment(&[7; 10], |offset| {
        vec![
            [
                tfhd(9, 0, &[]),
                tfdt(0, 0),
                trun(0, SIZE | DATA_OFFSET, offset, None, &[[0, 10, 0, 0]]),
            ]
            .concat(),
        ]
    });
    assert!(matches!(
        parse_segment(&bytes, &video_tracks(None)),
        Err(Error::Malformed(_))
    ));
}

/** A trun declaring more samples than the limit is refused before anything is allocated for them. */
#[test]
fn segment_refuses_a_sample_count_beyond_the_limit() {
    let bytes = media_segment(&[7; 10], |offset| {
        let mut payload = u32s(&[u32::MAX]);
        payload.extend_from_slice(&offset.to_be_bytes());
        let run = fb(b"trun", 0, DATA_OFFSET, &payload);
        vec![[tfhd(1, 0, &[]), tfdt(0, 0), run].concat()]
    });
    assert!(matches!(
        parse_segment(&bytes, &video_tracks(None)),
        Err(Error::Malformed(_))
    ));
}

/** A segment with no moof has no samples, and an empty one is empty. */
#[test]
fn segment_without_a_moof_has_no_samples() {
    let bytes = bx(b"mdat", &[1, 2, 3]);
    assert!(
        parse_segment(&bytes, &video_tracks(None))
            .unwrap()
            .is_empty()
    );
}

/** A trun of `count` samples with no per-sample fields: a tfhd default size of one byte, every sample at `offset`'s data. */
fn repeated_run(count: u32, offset: i32) -> Vec<u8> {
    let mut payload = u32s(&[count]);
    payload.extend_from_slice(&offset.to_be_bytes());
    [
        tfhd(1, TFHD_DEFAULT_SIZE, &[1]),
        tfdt(0, 0),
        fb(b"trun", 0, DATA_OFFSET, &payload),
    ]
    .concat()
}

/** Each trun re-reads the same bytes, so three runs of a thousand samples each exceed a segment of about 1.2 KB. */
#[test]
fn segment_refuses_runs_that_repeat_their_data_beyond_the_budget() {
    let bytes = media_segment(&[7; 1000], |offset| {
        (0..3).map(|_| repeated_run(1000, offset)).collect()
    });
    assert_eq!(
        parse_segment(&bytes, &video_tracks(None)),
        Err(Error::Malformed("sample count"))
    );
}

/** A single run within the budget is accepted: the budget bounds the total, it does not ban large runs. */
#[test]
fn segment_accepts_one_run_within_the_budget() {
    let bytes = media_segment(&[7; 1000], |offset| vec![repeated_run(1000, offset)]);
    assert_eq!(
        parse_segment(&bytes, &video_tracks(None)).unwrap().len(),
        1000
    );
}

/** A zero-size sample names no bytes, so it is refused: a hostile run could otherwise repeat it without cost. */
#[test]
fn segment_refuses_a_zero_size_sample() {
    let bytes = media_segment(&[7; 10], |offset| {
        vec![
            [
                tfhd(1, 0, &[]),
                tfdt(0, 0),
                trun(0, SIZE | DATA_OFFSET, offset, None, &[[0, 0, 0, 0]]),
            ]
            .concat(),
        ]
    });
    assert_eq!(
        parse_segment(&bytes, &video_tracks(None)),
        Err(Error::Malformed("zero-size sample"))
    );
}
