/** Tests of CDEF: skipped and unindexed blocks keep their samples, the constrain function damps large differences away, the Airbnb picture (which turns CDEF off) is untouched, and fixtures that enable it change in every plane. */
use super::cdef_frame;
use super::filter::constrain;
use crate::post::deblock_frame;
use crate::test_support::{AIRBNB, CHIMERA, MEXICO, decode_fixture};

/** A difference far past its threshold is damped to nothing; a small one passes almost whole, and the sign follows the difference. */
#[test]
fn constrain_damps_large_differences_to_zero() {
    assert_eq!(constrain(0, 4, 3), 0);
    assert_eq!(constrain(100, 4, 3), 0);
    assert_eq!(constrain(2, 4, 6), 2);
    assert_eq!(constrain(-2, 4, 6), -2);
}

/** A zero strength never moves a sample, whatever the difference. */
#[test]
fn a_zero_threshold_contributes_nothing() {
    assert_eq!(constrain(50, 0, 6), 0);
}

/** A frame whose blocks are all skipped is copied through CDEF unchanged. */
#[test]
fn skipped_blocks_are_copied_unchanged() {
    let mut d = decode_fixture(AIRBNB);
    let (rows, cols) = (d.hdr.size.mi_rows as usize, d.hdr.size.mi_cols as usize);
    d.fs.mi.fill(0, 0, rows, cols, |m| m.skip = true);
    let out = cdef_frame(&d.seq, &d.hdr, &d.fs);
    assert_eq!(out[0], d.fs.planes[0]);
}

/** Units with no CDEF index are copied through unchanged, even when their blocks are coded. */
#[test]
fn units_without_an_index_are_copied_unchanged() {
    let mut d = decode_fixture(AIRBNB);
    let (rows, cols) = (d.hdr.size.mi_rows as usize, d.hdr.size.mi_cols as usize);
    for r in (0..rows).step_by(16) {
        for c in (0..cols).step_by(16) {
            d.fs.cdef.set(r, c, -1);
        }
    }
    let out = cdef_frame(&d.seq, &d.hdr, &d.fs);
    assert_eq!(out[0], d.fs.planes[0]);
}

/** The Airbnb picture has CDEF switched off in its sequence header, so CDEF must leave its samples alone. */
#[test]
fn cdef_is_off_for_the_airbnb_picture() {
    let mut d = decode_fixture(AIRBNB);
    deblock_frame(&d.seq, &d.hdr, &mut d.fs);
    let out = cdef_frame(&d.seq, &d.hdr, &d.fs);
    assert!(!d.seq.enable_cdef);
    assert_eq!(out[0], d.fs.planes[0]);
}

/** Fixtures that enable CDEF have their samples changed by it, in every plane they carry, which shows the stage runs on real strengths. */
#[test]
fn cdef_changes_samples_of_fixtures_that_enable_it() {
    for (name, bytes) in [("chimera-8bit", CHIMERA), ("mexico-yuv444", MEXICO)] {
        let mut d = decode_fixture(bytes);
        assert!(d.seq.enable_cdef, "{name} enables CDEF");
        deblock_frame(&d.seq, &d.hdr, &mut d.fs);
        let out = cdef_frame(&d.seq, &d.hdr, &d.fs);
        for (plane, (got, before)) in out
            .iter()
            .zip(&d.fs.planes)
            .enumerate()
            .take(d.seq.color.num_planes())
        {
            assert_ne!(got, before, "{name} plane {plane}");
        }
    }
}
