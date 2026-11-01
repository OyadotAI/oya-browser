/** Initialization segments through the public surface: track ids, timescales, kinds, codec strings, defaults, edits and refusals. */
mod support;

use mars_mp4::{Error, TrackKind, parse_init};
use support::{TrackSpec, bx, init_segment};

/** Each track's id and timescale come from its own tkhd and mdhd. */
#[test]
fn init_reads_each_track_id_and_timescale() {
    let tracks = parse_init(&init_segment(&[TrackSpec::video(1), TrackSpec::audio(2)])).unwrap();
    let ids: Vec<(u32, u32)> = tracks
        .iter()
        .map(|track| (track.id, track.timescale))
        .collect();
    assert_eq!(ids, vec![(1, 1000), (2, 48000)]);
}

/** The handler decides the kind: vide is video, soun is audio. */
#[test]
fn init_takes_the_track_kind_from_its_handler() {
    let tracks = parse_init(&init_segment(&[TrackSpec::video(1), TrackSpec::audio(2)])).unwrap();
    assert_eq!(tracks[0].kind, TrackKind::Video);
    assert_eq!(tracks[1].kind, TrackKind::Audio);
}

/** An AVC track's codec string is avc1 with the profile, compatibility and level bytes of its avcC. */
#[test]
fn init_names_an_avc_track_from_its_avcc_record() {
    let tracks = parse_init(&init_segment(&[TrackSpec::video(1)])).unwrap();
    assert_eq!(tracks[0].codec, "avc1.64001F");
}

/** An AAC track's codec string carries the audio object type from its DecoderSpecificInfo. */
#[test]
fn init_names_an_aac_track_from_its_object_type() {
    let tracks = parse_init(&init_segment(&[TrackSpec::audio(2)])).unwrap();
    assert_eq!(tracks[0].codec, "mp4a.40.2");
}

/** The trex defaults for the track's id are carried onto the track. */
#[test]
fn init_carries_each_tracks_trex_defaults() {
    let spec = TrackSpec {
        trex: [512, 900, 0x0101_0000],
        ..TrackSpec::video(3)
    };
    let tracks = parse_init(&init_segment(&[spec])).unwrap();
    assert_eq!(
        (
            tracks[0].default_duration,
            tracks[0].default_size,
            tracks[0].default_flags
        ),
        (512, 900, 0x0101_0000)
    );
}

/** The first edit's media time is the offset presentation times subtract. */
#[test]
fn init_takes_the_first_edit_media_time_as_the_offset() {
    let spec = TrackSpec {
        media_time: Some(1024),
        ..TrackSpec::video(1)
    };
    let tracks = parse_init(&init_segment(&[spec])).unwrap();
    assert_eq!(tracks[0].edit_offset, 1024);
}

/** A track with no edit list has no offset. */
#[test]
fn init_uses_no_edit_offset_without_an_edit_list() {
    let tracks = parse_init(&init_segment(&[TrackSpec::video(1)])).unwrap();
    assert_eq!(tracks[0].edit_offset, 0);
}

/** A segment with no moov is refused as missing it. */
#[test]
fn init_refuses_a_segment_without_moov() {
    let bytes = bx(b"ftyp", b"iso5");
    assert_eq!(parse_init(&bytes), Err(Error::Missing(*b"moov")));
}

/** A box whose declared size runs past the input is refused as truncated, not read past its end. */
#[test]
fn init_refuses_a_box_cut_short() {
    let mut bytes = init_segment(&[TrackSpec::video(1)]);
    bytes.truncate(bytes.len() - 20);
    assert!(matches!(parse_init(&bytes), Err(Error::Truncated(_))));
}
