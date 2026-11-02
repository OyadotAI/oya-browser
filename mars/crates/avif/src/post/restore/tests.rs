/** Tests of loop restoration: an identity Wiener unit leaves its samples alone, the coded units of the fixtures that use restoration change their chroma, and the Airbnb picture, which codes none, is untouched. */
use super::restore_frame;
use crate::av1::RESTORE_WIENER;
use crate::post::{cdef_frame, deblock_frame};
use crate::test_support::{AIRBNB, CHIMERA, decode_fixture};

/** A Wiener unit whose three coded taps are all zero is the identity: the centre tap is 128, which unit gain turns into no change. Restoring a frame with such units everywhere returns its CDEF samples exactly. */
#[test]
fn an_identity_wiener_unit_leaves_its_samples_unchanged() {
    let mut d = decode_fixture(CHIMERA);
    deblock_frame(&d.seq, &d.hdr, &mut d.fs);
    let cdef = cdef_frame(&d.seq, &d.hdr, &d.fs);
    for plane in &mut d.fs.lr {
        for unit in &mut plane.units {
            unit.kind = RESTORE_WIENER;
            unit.wiener = [[0; 3]; 2];
        }
    }
    let out = restore_frame(&d.seq, &d.hdr, &d.fs, &d.fs.planes, &cdef);
    assert_eq!(out, cdef);
}

/** Restoration on the Chimera fixture changes its chroma, which carries Wiener units; the picture has no restoration on luma. */
#[test]
fn coded_restoration_changes_the_chroma_of_chimera() {
    let mut d = decode_fixture(CHIMERA);
    deblock_frame(&d.seq, &d.hdr, &mut d.fs);
    let cdef = cdef_frame(&d.seq, &d.hdr, &d.fs);
    let out = restore_frame(&d.seq, &d.hdr, &d.fs, &d.fs.planes, &cdef);
    assert_eq!(out[0], cdef[0], "luma has no restoration");
    assert_ne!(out[1], cdef[1], "chroma u is restored");
    assert_ne!(out[2], cdef[2], "chroma v is restored");
}

/** The Airbnb picture codes no restoration, so its frame passes through unchanged. */
#[test]
fn the_airbnb_picture_has_no_restoration() {
    let mut d = decode_fixture(AIRBNB);
    deblock_frame(&d.seq, &d.hdr, &mut d.fs);
    let cdef = cdef_frame(&d.seq, &d.hdr, &d.fs);
    let out = restore_frame(&d.seq, &d.hdr, &d.fs, &d.fs.planes, &cdef);
    assert!(!d.hdr.lr.uses_lr);
    assert_eq!(out, cdef);
}

/** A flat picture is a fixed point of the self-guided filter for any valid weights, since the weights sum to 2^SGRPROJ_PRJ_BITS and both estimates of a flat region equal it. */
#[test]
fn a_flat_picture_is_a_fixed_point_of_self_guided_restoration() {
    use crate::av1::RESTORE_SGRPROJ;
    use crate::recon::Plane;
    let mut d = decode_fixture(CHIMERA);
    let flat = |p: &Plane| {
        let mut q = p.clone();
        for y in 0..p.height() {
            for x in 0..p.width() {
                q.set(x, y, 100);
            }
        }
        q
    };
    let cdef = [
        flat(&d.fs.planes[0]),
        flat(&d.fs.planes[1]),
        flat(&d.fs.planes[2]),
    ];
    for plane in &mut d.fs.lr {
        for unit in &mut plane.units {
            unit.kind = RESTORE_SGRPROJ;
            unit.sgr_set = 0;
            unit.sgr_xqd = [-32, 31];
        }
    }
    let out = restore_frame(&d.seq, &d.hdr, &d.fs, &cdef, &cdef);
    assert_eq!(out[1].get(8, 8), 100, "chroma u stays flat");
    assert_eq!(out[0].get(8, 8), 100, "luma stays flat");
}
