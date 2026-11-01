/** Reconstruction tests: each 1-D transform matches a floating-point reference within rounding, the 2-D transform of a DC-only block is flat and equals its hand-derived value, the lossless transform has exact vectors, and the residual lands with the flips applied. */
use super::adst;
use super::dct;
use super::identity;
use super::inverse::{Geometry, inverse_2d};
use super::reconstruct::add_residual;
use super::txkind::tx_kinds;
use super::{Plane, ResidualSpec, reconstruct};
use std::f64::consts::PI;

/** A deterministic pseudo-random sequence in a range, so that the tests are reproducible without a random crate. */
fn lcg(seed: &mut u64, range: i32) -> i32 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*seed >> 33) as i32).rem_euclid(2 * range + 1) - range
}

/** Inverse DCT of length n in floating point: the DC term carries 1/sqrt(2), the rest are cosines. */
fn dct_reference(input: &[f64]) -> Vec<f64> {
    let n = input.len();
    (0..n)
        .map(|i| {
            let mut sum = input[0] / 2f64.sqrt();
            for (k, value) in input.iter().enumerate().skip(1) {
                sum += value * (PI * (2 * i + 1) as f64 * k as f64 / (2 * n) as f64).cos();
            }
            sum
        })
        .collect()
}

/** Inverse ADST of length 8 or 16 in floating point: sines at odd multiples of a quarter of the period. */
fn adst_reference(input: &[f64]) -> Vec<f64> {
    let n = input.len();
    (0..n)
        .map(|i| {
            input
                .iter()
                .enumerate()
                .map(|(k, value)| {
                    value * (PI * (2 * i + 1) as f64 * (2 * k + 1) as f64 / (4 * n) as f64).sin()
                })
                .sum()
        })
        .collect()
}

/** Inverse ADST4 in floating point: the 4-point sine basis with the 2*sqrt(2)/3 scale of the integer constants. */
fn adst4_reference(input: &[f64]) -> Vec<f64> {
    (0..4)
        .map(|i| {
            input
                .iter()
                .enumerate()
                .map(|(k, value)| {
                    value
                        * (2.0 * 2f64.sqrt() / 3.0)
                        * (PI * (2 * k + 1) as f64 * (i + 1) as f64 / 9.0).sin()
                })
                .sum()
        })
        .collect()
}

/** The largest absolute difference between an integer transform output and its reference. */
fn max_error(got: &[i32], want: &[f64]) -> f64 {
    got.iter()
        .zip(want)
        .map(|(g, w)| (f64::from(*g) - w).abs())
        .fold(0.0, f64::max)
}

/** Each DCT length matches its floating-point reference on random inputs, to within the rounding that twelve-bit butterflies accumulate. */
#[test]
fn dct_matches_the_float_reference_for_every_length() {
    let mut seed = 7;
    for n in 2..=6u32 {
        let len = 1usize << n;
        for _ in 0..50 {
            let input: Vec<i32> = (0..len).map(|_| lcg(&mut seed, 500)).collect();
            let mut t = [0i32; 64];
            t[..len].copy_from_slice(&input);
            dct::inverse(&mut t, n, 24);
            let want = dct_reference(&input.iter().map(|v| f64::from(*v)).collect::<Vec<_>>());
            let error = max_error(&t[..len], &want);
            assert!(error <= 1.5 * f64::from(n), "DCT{len} off by {error}");
        }
        for k in 0..len {
            let mut t = [0i32; 64];
            t[k] = 1000;
            dct::inverse(&mut t, n, 24);
            let mut input = vec![0f64; len];
            input[k] = 1000.0;
            let error = max_error(&t[..len], &dct_reference(&input));
            assert!(error <= 2.0, "DCT{len} impulse {k} off by {error}");
        }
    }
}

/** The 8- and 16-point ADSTs match the sine reference on random inputs. */
#[test]
fn adst_matches_the_float_reference_for_eight_and_sixteen() {
    let mut seed = 11;
    for n in 3..=4u32 {
        let len = 1usize << n;
        for _ in 0..50 {
            let input: Vec<i32> = (0..len).map(|_| lcg(&mut seed, 500)).collect();
            let mut t = [0i32; 64];
            t[..len].copy_from_slice(&input);
            adst::inverse(&mut t, n, 24);
            let want = adst_reference(&input.iter().map(|v| f64::from(*v)).collect::<Vec<_>>());
            let error = max_error(&t[..len], &want);
            assert!(error <= 2.0 + n as f64, "ADST{len} off by {error}");
        }
    }
}

/** The 4-point ADST matches the sine reference on random inputs. */
#[test]
fn adst4_matches_the_float_reference() {
    let mut seed = 13;
    for _ in 0..100 {
        let input: Vec<i32> = (0..4).map(|_| lcg(&mut seed, 500)).collect();
        let mut t = [0i32; 4];
        t.copy_from_slice(&input);
        adst::inverse4(&mut t);
        let want = adst4_reference(&input.iter().map(|v| f64::from(*v)).collect::<Vec<_>>());
        assert!(max_error(&t, &want) <= 2.0, "ADST4 off: {t:?} vs {want:?}");
    }
}

/** A DCT of a single coefficient at the DC position spreads it evenly: the 4-point case is the value times 2896/4096 at every output. */
#[test]
fn dct4_dc_only_is_flat_and_known() {
    let mut t = [0i32; 64];
    t[0] = 4096;
    dct::inverse(&mut t, 2, 24);
    assert_eq!(&t[..4], &[2896, 2896, 2896, 2896]);
}

/** The identity transforms scale by their size factors: 4 by sqrt(2) with rounding, 8 by 2, 16 by 2*sqrt(2) with rounding, 32 by 4. */
#[test]
fn identity_scales_by_size() {
    let mut t = [1000i32; 32];
    identity::inverse(&mut t, 2);
    assert_eq!(t[0], 1414);
    let mut t = [1000i32; 32];
    identity::inverse(&mut t, 3);
    assert_eq!(t[0], 2000);
    let mut t = [1000i32; 32];
    identity::inverse(&mut t, 4);
    assert_eq!(t[0], 2829);
    let mut t = [1000i32; 32];
    identity::inverse(&mut t, 5);
    assert_eq!(t[0], 4000);
}

/** The Walsh-Hadamard transform of a lone DC input of 4 with the lossless pre-shift of 2 gives a lone DC of 1, and of zero gives zero. */
#[test]
fn wht_of_a_lone_dc_is_exact() {
    let mut t = [4, 0, 0, 0];
    identity::wht(&mut t, 2);
    assert_eq!(t, [1, 0, 0, 0]);
    let mut t = [0; 4];
    identity::wht(&mut t, 2);
    assert_eq!(t, [0; 4]);
}

/** A DCT_DCT block whose only coefficient is the DC is flat, and its value is the hand-derived one: the row pass, its shift, the column pass, and the final shift of four. */
#[test]
fn dc_only_dct_block_is_flat_with_the_expected_value() {
    let geom = Geometry {
        log2w: 3,
        log2h: 3,
        row_shift: 1,
    };
    let mut coef = vec![0i32; 64];
    coef[0] = 1024;
    let res = inverse_2d(&coef, geom, tx_kinds(0), false, 8);
    let row = (1024i64 * 2896 + 2048) >> 12;
    let row = (row + 1) >> 1;
    let col = (row * 2896 + 2048) >> 12;
    let expected = (col + 8) >> 4;
    assert!(
        res.iter().all(|v| i64::from(*v) == expected),
        "{res:?} != {expected}"
    );
}

/** A rectangular 8x4 block applies the 1/sqrt(2) pre-scale to its rows, so its DC value differs from the square one by that factor. */
#[test]
fn rectangular_block_prescales_its_rows() {
    let geom = Geometry {
        log2w: 3,
        log2h: 2,
        row_shift: 1,
    };
    let mut coef = vec![0i32; 32];
    coef[0] = 1024;
    let res = inverse_2d(&coef, geom, tx_kinds(0), false, 8);
    let scaled = (1024i64 * 2896 + 2048) >> 12;
    let row = (scaled * 2896 + 2048) >> 12;
    let row = (row + 1) >> 1;
    let col = (row * 2896 + 2048) >> 12;
    let expected = (col + 8) >> 4;
    assert!(res.iter().all(|v| i64::from(*v) == expected));
}

/** A lossless 4x4 block with one DC coefficient reconstructs to a flat residual through the Walsh-Hadamard transform. */
#[test]
fn lossless_dc_block_is_flat() {
    let geom = Geometry {
        log2w: 2,
        log2h: 2,
        row_shift: 0,
    };
    let mut coef = vec![0i32; 16];
    coef[0] = 16;
    let res = inverse_2d(&coef, geom, tx_kinds(0), true, 8);
    assert_eq!(res.len(), 16);
    assert!(res.iter().all(|v| *v == res[0]));
}

/** A vertically flipped residual lands with its first row on the last row of the block, and a horizontally flipped one lands its first column on the last column. */
#[test]
fn flips_move_the_residual_into_place() {
    let residual: Vec<i32> = (0..16).collect();
    let geom = Geometry {
        log2w: 2,
        log2h: 2,
        row_shift: 0,
    };
    let mut plane = Plane::new(4, 4);
    let flip_ud = ResidualSpec {
        x: 0,
        y: 0,
        geom,
        tx_type: 4,
        lossless: false,
        bit_depth: 8,
    };
    add_residual(&mut plane, &flip_ud, &residual);
    assert_eq!((plane.get(0, 3), plane.get(3, 3)), (0, 3));
    assert_eq!((plane.get(0, 0), plane.get(3, 0)), (12, 15));
    let mut plane = Plane::new(4, 4);
    let flip_lr = ResidualSpec {
        tx_type: 5,
        ..flip_ud
    };
    add_residual(&mut plane, &flip_lr, &residual);
    assert_eq!((plane.get(3, 0), plane.get(0, 0)), (0, 3));
    assert_eq!((plane.get(0, 3), plane.get(3, 3)), (15, 12));
}

/** Samples added by a residual are clipped to the bit depth, so an overflowing coefficient saturates rather than wrapping. */
#[test]
fn reconstruction_saturates_to_the_bit_depth() {
    let mut plane = Plane::new(4, 4);
    for y in 0..4 {
        plane.row_mut(y).fill(250);
    }
    let spec = ResidualSpec {
        x: 0,
        y: 0,
        geom: Geometry {
            log2w: 2,
            log2h: 2,
            row_shift: 0,
        },
        tx_type: 0,
        lossless: false,
        bit_depth: 8,
    };
    reconstruct(&mut plane, &spec, &[(0, 30000)]);
    assert!((0..4).all(|y| (0..4).all(|x| plane.get(x, y) == 255)));
}
