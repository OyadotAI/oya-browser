use super::filter_frame;
/** Tests of the deblocking filter: a step too large to be an edge is kept, a small step is smoothed, a flat line does not move, a zero level leaves the frame alone, and the Airbnb picture gets closer to its JPEG. */
use super::filters::{Cfg, Sample, filter_sample};
use super::level::Strength;
use crate::recon::Plane;
use crate::test_support::{AIRBNB, decode_fixture, jpeg_luma, picture_luma, psnr};

/** A one-row plane with `left` samples then `right` samples, split at the given column. */
fn step_plane(split: usize, left: u16, right: u16) -> Plane {
    let mut plane = Plane::new(16, 1);
    for x in 0..16 {
        plane.set(x, 0, if x < split { left } else { right });
    }
    plane
}

/** A strength with the given level and the limits it implies at sharpness zero. */
fn strength(lvl: i32) -> Strength {
    Strength {
        lvl,
        limit: lvl.max(1),
        blimit: 2 * (lvl + 2) + lvl.max(1),
        thresh: lvl >> 4,
    }
}

/** The 8-bit filter settings for a plane of the given size. */
fn cfg(size: usize) -> Cfg {
    Cfg {
        size,
        chroma: false,
        bit_depth: 8,
    }
}

/** A vertical edge at x = 8 in the one-row plane. */
fn edge_site() -> Sample {
    Sample {
        x: 8,
        y: 0,
        dx: 1,
        dy: 0,
    }
}

/** A step of 100 between the two sides exceeds every limit of a level-4 filter, so the mask refuses it and the samples stay as they were. */
#[test]
fn a_step_too_large_to_be_an_edge_is_left_alone() {
    let mut plane = step_plane(8, 10, 110);
    let before = plane.row(0).to_vec();
    filter_sample(&mut plane, edge_site(), cfg(4), &strength(4));
    assert_eq!(plane.row(0), before.as_slice());
}

/** A step of four under a level-32 filter is smoothed: the samples either side move towards each other. */
#[test]
fn a_small_step_across_an_edge_is_smoothed() {
    let mut plane = step_plane(8, 100, 104);
    filter_sample(&mut plane, edge_site(), cfg(4), &strength(32));
    let row = plane.row(0);
    assert!(row[7] > 100, "p0 rose to {}", row[7]);
    assert!(row[8] < 104, "q0 fell to {}", row[8]);
}

/** A flat line has no step to smooth, so every filter leaves it unchanged. */
#[test]
fn a_flat_line_is_unchanged_by_every_filter_size() {
    for size in [4, 8, 16] {
        let mut plane = step_plane(8, 77, 77);
        filter_sample(&mut plane, edge_site(), cfg(size), &strength(63));
        assert!(plane.row(0).iter().all(|&v| v == 77), "size {size}");
    }
}

/** A frame whose loop-filter levels are both zero is not touched at all, whatever the blocks say. */
#[test]
fn a_frame_with_zero_loop_filter_levels_is_untouched() {
    let mut decoded = decode_fixture(AIRBNB);
    decoded.hdr.loop_filter.level = [0; 4];
    let before = decoded.fs.planes[0].clone();
    filter_frame(&decoded.seq, &decoded.hdr, &mut decoded.fs);
    assert_eq!(decoded.fs.planes[0], before);
}

/** Deblocking the Airbnb picture never moves its luma further from the JPEG than the unfiltered reconstruction is, and the filtered picture stays a close match. */
#[test]
fn deblocking_keeps_airbnb_luma_at_least_as_close_to_the_jpeg() {
    let mut decoded = decode_fixture(AIRBNB);
    let (want, _, _) = jpeg_luma();
    let before = psnr(&picture_luma(&decoded.fs.planes[0], &decoded.hdr), &want);
    filter_frame(&decoded.seq, &decoded.hdr, &mut decoded.fs);
    let after = psnr(&picture_luma(&decoded.fs.planes[0], &decoded.hdr), &want);
    eprintln!("airbnb luma PSNR: before deblock {before:.2} dB, after {after:.2} dB");
    assert!(
        after >= before,
        "deblocking lowered PSNR {before:.2} -> {after:.2}"
    );
    assert!(after > 25.0, "PSNR {after:.2} dB");
}
