/** Intra prediction tests: the edge substitutions at the frame and decode-order borders, the directional and non-directional predictors on known edges, the edge-filter strength and upsampling rules, and the palette lookup. */
use super::edges::build;
use super::filter::{strength, use_upsample};
use super::{PaletteBlock, Request, predict, predict_palette};
use crate::recon::Plane;

/** A request for a block of 2^log2 samples at (x, y) with every neighbour available or not as given, and the plane's last sample at the given corner. */
fn request(x: usize, y: usize, log2: u32, have: (bool, bool), mode: u8) -> Request {
    Request {
        x,
        y,
        have_left: have.0,
        have_above: have.1,
        have_above_right: false,
        have_below_left: false,
        mode,
        log2w: log2,
        log2h: log2,
        angle_delta: 0,
        filter_intra: None,
        edge_filter: false,
        smooth_neighbour: false,
        max_x: 63,
        max_y: 63,
        bit_depth: 8,
    }
}

/** A plane whose samples are a horizontal ramp, so that a predictor's source column is visible in its output. */
fn ramp() -> Plane {
    let mut plane = Plane::new(64, 64);
    for y in 0..64 {
        for x in 0..64 {
            plane.set(x, y, (x * 2) as u16);
        }
    }
    plane
}

/** With no neighbour on either side, the edges are the mid grey of the bit depth, one above and one below, so DC predicts 128. */
#[test]
fn dc_without_neighbours_is_mid_grey() {
    let mut plane = Plane::new(64, 64);
    predict(&mut plane, &request(0, 0, 2, (false, false), 0));
    assert!((0..4).all(|y| (0..4).all(|x| plane.get(x, y) == 128)));
}

/** A vertical prediction copies the row above the block, so each column takes the sample above it. */
#[test]
fn vertical_copies_the_row_above() {
    let mut plane = ramp();
    predict(&mut plane, &request(8, 8, 2, (true, true), 1));
    for y in 8..12 {
        for x in 8..12 {
            assert_eq!(plane.get(x, y), (x * 2) as i32);
        }
    }
}

/** A horizontal prediction at the left frame edge, with no column to the left, copies the row above's first sample along each row. */
#[test]
fn horizontal_without_a_left_column_repeats_the_corner_row() {
    let mut plane = ramp();
    predict(&mut plane, &request(0, 8, 2, (false, true), 2));
    assert!((0..4).all(|x| plane.get(x, 9) == plane.get(0, 7)));
}

/** A smooth predictor over constant edges predicts that constant everywhere, for all three smooth modes. */
#[test]
fn smooth_over_constant_edges_is_constant() {
    for mode in [9u8, 10, 11] {
        let mut plane = Plane::new(64, 64);
        for y in 0..64 {
            plane.row_mut(y).fill(77);
        }
        predict(&mut plane, &request(8, 8, 3, (true, true), mode));
        assert!(
            (8..16).all(|y| (8..16).all(|x| plane.get(x, y) == 77)),
            "mode {mode}"
        );
    }
}

/** With only a left neighbour, the above row and the corner repeat the left column's first sample, which is the spec's substitution when the row above is missing. */
#[test]
fn left_only_edges_repeat_the_left_sample() {
    let plane = ramp();
    let req = request(4, 0, 2, (true, false), 0);
    let e = build(&plane, &req);
    assert_eq!(e.above(0), plane.get(3, 0));
    assert_eq!(e.above(-1), plane.get(3, 0));
    assert_eq!(e.left(0), plane.get(3, 0));
}

/** The edge-filter strength follows the spec's thresholds: a near-vertical direction on a large block is filtered the most, and a small block with a small angle difference is not filtered at all. */
#[test]
fn edge_strength_follows_the_size_and_angle_thresholds() {
    assert_eq!(strength(4, 4, false, 60), 1);
    assert_eq!(strength(4, 4, false, 10), 0);
    assert_eq!(strength(32, 32, false, 0), 3);
    assert_eq!(strength(8, 8, true, 64), 2);
    assert_eq!(strength(8, 8, true, 39), 1);
    assert_eq!(strength(8, 8, true, 10), 0);
}

/** Upsampling applies only to short edges, and only for a direction that is off the nominal angle by less than forty degrees. */
#[test]
fn upsampling_needs_a_short_edge_and_a_near_angle() {
    assert!(use_upsample(4, 4, false, 10));
    assert!(!use_upsample(4, 4, false, 0));
    assert!(!use_upsample(4, 4, false, 40));
    assert!(!use_upsample(16, 16, false, 10));
    assert!(use_upsample(4, 4, true, 10));
    assert!(!use_upsample(8, 8, true, 10));
}

/** A palette block's samples are the palette colours its map names, read at the transform block's offset inside the block. */
#[test]
fn palette_prediction_looks_up_the_colour_of_each_index() {
    let colors = [10u16, 20, 30, 40, 50, 60, 70, 80];
    let mut map = vec![0u8; 64 * 64];
    for (i, slot) in map.iter_mut().enumerate() {
        *slot = (i % 8) as u8;
    }
    let block = PaletteBlock {
        colors: &colors,
        map: &map,
        stride: 64,
        x4: 1,
        y4: 0,
    };
    let mut plane = Plane::new(16, 16);
    predict_palette(&mut plane, 4, 4, 4, 4, &block);
    assert_eq!(plane.get(4, 4), 50);
    assert_eq!(plane.get(5, 4), 60);
}
