/** Tests of the output conversion: the range maps put black and white where they belong, a neutral chroma gives equal channels under every matrix, and chroma between two samples is their average. */
use super::matrix::{Matrix, Range};
use super::{Layout, chroma_at, from_weights};
use crate::recon::Plane;

/** Limited range maps 16 to black and 235 to white; full range maps 0 and 255 to the ends. */
#[test]
fn limited_range_maps_16_to_black_and_235_to_white() {
    let limited = Range {
        full: false,
        bit_depth: 8,
    };
    assert_eq!(limited.luma(16), 0.0);
    assert!((limited.luma(235) - 1.0).abs() < 1e-12);
    let full = Range {
        full: true,
        bit_depth: 8,
    };
    assert_eq!(full.luma(0), 0.0);
    assert!((full.luma(255) - 1.0).abs() < 1e-12);
}

/** Neutral chroma (Cb = Cr = 0) gives the luma in every channel, whichever luma weights the matrix uses. */
#[test]
fn neutral_chroma_is_grey_under_every_matrix() {
    for (kr, kb) in [(0.299, 0.114), (0.2126, 0.0722), (0.2627, 0.0593)] {
        let rgb = from_weights(0.4, 0.0, 0.0, kr, kb);
        for channel in rgb {
            assert!((channel - 0.4).abs() < 1e-12, "{rgb:?}");
        }
    }
}

/** The identity matrix names no weights: it is recognised by its code and never mixes channels. */
#[test]
fn the_identity_code_selects_the_identity_matrix() {
    assert_eq!(Matrix::from_code(crate::av1::MC_IDENTITY), Matrix::Identity);
    assert!(matches!(Matrix::from_code(9), Matrix::Weights { .. }));
}

/** Between two chroma sample centres the code is weighted by distance, so a chroma edge is a ramp rather than a step. */
#[test]
fn chroma_between_two_samples_is_interpolated_by_distance() {
    let mut plane = Plane::new(4, 4);
    for y in 0..4 {
        plane.set(0, y, 100);
        plane.set(1, y, 200);
        plane.set(2, y, 200);
        plane.set(3, y, 200);
    }
    let layout = Layout {
        sub_x: 1,
        sub_y: 0,
        mono: false,
        matrix: Matrix::Identity,
        last: (1, 3),
    };
    // Luma column 1 sits a quarter of the way from chroma column 0's centre to column 1's, so its code is 100 + 0.25 * 100.
    let value = chroma_at(&plane, (1, 0), layout);
    assert!((value - 125.0).abs() < 1e-9, "got {value}");
}

/** A rectangle with no samples has no last sample, and an origin that would overflow is refused too; neither wraps around to a huge index. */
#[test]
fn an_empty_or_overflowing_run_has_no_last_sample() {
    use super::last_index;
    use crate::error::Error;
    assert_eq!(
        last_index(0, 0),
        Err(Error::Malformed("empty output rectangle"))
    );
    assert_eq!(
        last_index(5, 0),
        Err(Error::Malformed("empty output rectangle"))
    );
    assert_eq!(
        last_index(usize::MAX, 2),
        Err(Error::Malformed("empty output rectangle"))
    );
    assert_eq!(last_index(3, 4), Ok(6));
}
