/*! The fdlibm algorithms V8 keeps in src/base/ieee754.cc for exp, log, log2, log10, cbrt, tan
and tanh, ported step for step so every last bit matches Chrome. The system libm and musl's
port replaced several of these (exp, log, log2 and pow with ARM's routines, cbrt and log10 with
FreeBSD's newer ones, the tan kernel with a reworked one), and each replacement differs in the
last bit on some inputs, which is exactly what a page fingerprinting the engine looks for. */

#![allow(
    clippy::approx_constant,
    clippy::empty_line_after_doc_comments,
    clippy::eq_op
)]
#![allow(clippy::excessive_precision)]

/** The upper 32 bits of a double, signed, as fdlibm reads them. */
fn high(x: f64) -> i32 {
    (x.to_bits() >> 32) as i32
}

/** The lower 32 bits of a double. */
fn low(x: f64) -> u32 {
    x.to_bits() as u32
}

/** A double from its two words. */
fn from_words(hi: i32, lo: u32) -> f64 {
    f64::from_bits(((hi as u32 as u64) << 32) | u64::from(lo))
}

/** The double with its upper word replaced. */
fn with_high(x: f64, hi: i32) -> f64 {
    from_words(hi, low(x))
}

/** The double with its lower word replaced. */
fn with_low(x: f64, lo: u32) -> f64 {
    from_words(high(x), lo)
}

/** The high word of infinity and NaN. */
const INF_HIGH: i32 = 0x7FF0_0000;
/** The sign-stripped high-word mask. */
const ABS_MASK: i32 = 0x7FFF_FFFF;
/** The mantissa bits of a high word. */
const MANTISSA_MASK: i32 = 0x000F_FFFF;
/** The high word of 1.0. */
const ONE_HIGH: i32 = 0x3FF0_0000;
/** The exponent bias of a double. */
const BIAS: i32 = 1023;
/** 2^54, which scales a subnormal into the normal range. */
const TWO54: f64 = 1.801_439_850_948_198_4e16;
/** 2^24, the digit base of the pi/2 reduction. */
const TWO24: f64 = 1.677_721_6e7;
/** 2^-24. */
const TWON24: f64 = 5.960_464_477_539_062_5e-8;

/** 396 hex digits of 2/pi in 24-bit digits, for reducing huge arguments. */
const TWO_OVER_PI: [i32; 66] = [
    0xA2F983, 0x6E4E44, 0x1529FC, 0x2757D1, 0xF534DD, 0xC0DB62, 0x95993C, 0x439041, 0xFE5163,
    0xABDEBB, 0xC561B7, 0x246E3A, 0x424DD2, 0xE00649, 0x2EEA09, 0xD1921C, 0xFE1DEB, 0x1CB129,
    0xA73EE8, 0x8235F5, 0x2EBB44, 0x84E99C, 0x7026B4, 0x5F7E41, 0x3991D6, 0x398353, 0x39F49C,
    0x845F8B, 0xBDF928, 0x3B1FF8, 0x97FFDE, 0x05980F, 0xEF2F11, 0x8B5A0A, 0x6D1F6D, 0x367ECF,
    0x27CB09, 0xB74F46, 0x3F669E, 0x5FEA2D, 0x7527BA, 0xC7EBE5, 0xF17B3D, 0x0739F7, 0x8A5292,
    0xEA6BFB, 0x5FB11F, 0x8D5D08, 0x560330, 0x46FC7B, 0x6BABF0, 0xCFBC20, 0x9AF436, 0x1DA9E3,
    0x91615E, 0xE61B08, 0x659985, 0x5F14A0, 0x68408D, 0xFFD880, 0x4D7327, 0x310606, 0x1556CA,
    0x73A8C9, 0x60E27B, 0xC08C6B,
];
/** The high words of n * pi/2 for n = 1..32, to spot cancellation in the medium reduction. */
const NPIO2_HW: [i32; 32] = [
    0x3FF921FB, 0x400921FB, 0x4012D97C, 0x401921FB, 0x401F6A7A, 0x4022D97C, 0x4025FDBB, 0x402921FB,
    0x402C463A, 0x402F6A7A, 0x4031475C, 0x4032D97C, 0x40346B9C, 0x4035FDBB, 0x40378FDB, 0x403921FB,
    0x403AB41B, 0x403C463A, 0x403DD85A, 0x403F6A7A, 0x40407E4C, 0x4041475C, 0x4042106C, 0x4042D97C,
    0x4043A28C, 0x40446B9C, 0x404534AC, 0x4045FDBB, 0x4046C6CB, 0x40478FDB, 0x404858EB, 0x404921FB,
];
/** pi/2 in 24-bit pieces, for the huge-argument reduction. */
const PIO2: [f64; 8] = [
    1.570_796_251_296_997_070_31,
    7.549_789_415_861_596_353_35e-8,
    5.390_302_529_957_764_765_54e-15,
    3.282_003_415_807_912_941_23e-22,
    1.270_655_753_080_676_073_49e-29,
    1.229_333_089_811_113_289_32e-36,
    2.733_700_538_164_645_596_24e-44,
    2.167_416_838_778_048_194_44e-51,
];
/** 53 bits of 2/pi. */
const INVPIO2: f64 = 6.366_197_723_675_813_824_33e-1;
/** The first 33 bits of pi/2. */
const PIO2_1: f64 = 1.570_796_326_734_125_614_17;
/** pi/2 minus PIO2_1. */
const PIO2_1T: f64 = 6.077_100_506_506_192_249_32e-11;
/** The second 33 bits of pi/2. */
const PIO2_2: f64 = 6.077_100_506_303_965_976_60e-11;
/** pi/2 minus the first two pieces. */
const PIO2_2T: f64 = 2.022_266_248_795_950_631_54e-21;
/** The third 33 bits of pi/2. */
const PIO2_3: f64 = 2.022_266_248_711_166_455_80e-21;
/** pi/2 minus the first three pieces. */
const PIO2_3T: f64 = 8.478_427_660_368_899_569_97e-32;
/** The high word of pi/4: at or below it no reduction is needed. */
const PIO4_HIGH: i32 = 0x3FE9_21FB;
/** The high word of 3pi/4: below it the reduction is one subtraction. */
const THREE_PIO4_HIGH: i32 = 0x4002_D97C;
/** The high word of pi/2, where the one-subtraction case needs a third piece. */
const PIO2_HIGH: i32 = 0x3FF9_21FB;
/** The high word of 2^19 pi/2: at or below it the medium reduction suffices. */
const MEDIUM_HIGH: i32 = 0x4139_21FB;

/** Reduce x to y0 + y1 in [-pi/4, pi/4] and return the quadrant count, as fdlibm's
`__ieee754_rem_pio2` does: no reduction, one subtraction, a few multiplications, or the
multi-precision path for huge arguments. */
pub(super) fn rem_pio2(x: f64) -> (i32, f64, f64) {
    let hx = high(x);
    let ix = hx & ABS_MASK;
    if ix <= PIO4_HIGH {
        return (0, x, 0.0);
    }
    if ix < THREE_PIO4_HIGH {
        return reduce_once(x, hx, ix);
    }
    if ix <= MEDIUM_HIGH {
        return reduce_medium(x, hx, ix);
    }
    if ix >= INF_HIGH {
        return (0, x - x, x - x);
    }
    reduce_huge(x, hx, ix)
}

/** |x| < 3pi/4: subtract pi/2 once, with a third piece of pi near pi/2 itself. */
fn reduce_once(x: f64, hx: i32, ix: i32) -> (i32, f64, f64) {
    if hx > 0 {
        let z = x - PIO2_1;
        if ix != PIO2_HIGH {
            let y0 = z - PIO2_1T;
            return (1, y0, (z - y0) - PIO2_1T);
        }
        let z = z - PIO2_2;
        let y0 = z - PIO2_2T;
        return (1, y0, (z - y0) - PIO2_2T);
    }
    let z = x + PIO2_1;
    if ix != PIO2_HIGH {
        let y0 = z + PIO2_1T;
        return (-1, y0, (z - y0) + PIO2_1T);
    }
    let z = z + PIO2_2;
    let y0 = z + PIO2_2T;
    (-1, y0, (z - y0) + PIO2_2T)
}

/** |x| <= 2^19 pi/2: n pieces of pi/2, with a second and third round when the first cancels. */
fn reduce_medium(x: f64, hx: i32, ix: i32) -> (i32, f64, f64) {
    let t = x.abs();
    let n = (t * INVPIO2 + 0.5) as i32;
    let fnum = f64::from(n);
    let mut r = t - fnum * PIO2_1;
    let mut w = fnum * PIO2_1T;
    let mut y0 = r - w;
    if !(n < 32 && ix != NPIO2_HW[(n - 1) as usize]) {
        let j = ix >> 20;
        let mut i = j - ((high(y0) >> 20) & 0x7FF);
        if i > 16 {
            let t = r;
            w = fnum * PIO2_2;
            r = t - w;
            w = fnum * PIO2_2T - ((t - r) - w);
            y0 = r - w;
            i = j - ((high(y0) >> 20) & 0x7FF);
            if i > 49 {
                let t = r;
                w = fnum * PIO2_3;
                r = t - w;
                w = fnum * PIO2_3T - ((t - r) - w);
                y0 = r - w;
            }
        }
    }
    let y1 = (r - y0) - w;
    if hx < 0 { (-n, -y0, -y1) } else { (n, y0, y1) }
}

/** Every larger argument: split |x| into 24-bit digits and reduce against 2/pi. */
fn reduce_huge(x: f64, hx: i32, ix: i32) -> (i32, f64, f64) {
    let e0 = (ix >> 20) - 1046;
    let mut z = from_words(ix - (e0 << 20), low(x));
    let mut tx = [0.0; 3];
    for digit in tx.iter_mut().take(2) {
        *digit = f64::from(z as i32);
        z = (z - *digit) * TWO24;
    }
    tx[2] = z;
    let mut nx = 3;
    while tx[nx - 1] == 0.0 {
        nx -= 1;
    }
    let (n, y0, y1) = kernel_rem_pio2(&tx[..nx], e0);
    if hx < 0 { (-n, -y0, -y1) } else { (n, y0, y1) }
}

/** The number of 24-bit digits of 2/pi the two-double result needs (fdlibm's `init_jk[2]`). */
const JK: i32 = 4;
/** The working-array size fdlibm gives the reduction. */
const WORK: usize = 20;

/** The multi-precision core of the reduction (fdlibm's `__kernel_rem_pio2` at precision 2): the
digits of x times the digits of 2/pi, the integer part dropped modulo 8, and the fraction
scaled back by pi/2 into two doubles. */
fn kernel_rem_pio2(x: &[f64], e0: i32) -> (i32, f64, f64) {
    let jx = x.len() as i32 - 1;
    let jv = ((e0 - 3) / 24).max(0);
    let mut q0 = e0 - 24 * (jv + 1);
    let (mut f, mut q, mut iq) = ([0.0; WORK], [0.0; WORK], [0i32; WORK]);
    for i in 0..=(jx + JK) {
        let j = jv - jx + i;
        f[i as usize] = if j < 0 {
            0.0
        } else {
            f64::from(TWO_OVER_PI[j as usize])
        };
    }
    for i in 0..=JK {
        q[i as usize] = digit_product(x, &f, jx, i);
    }
    let mut jz = JK;
    let (mut n, mut ih, mut z);
    loop {
        let mut zz = q[jz as usize];
        for (i, j) in (1..=jz).rev().enumerate() {
            let fw = f64::from((TWON24 * zz) as i32);
            iq[i] = (zz - TWO24 * fw) as i32;
            zz = q[(j - 1) as usize] + fw;
        }
        z = libm::scalbn(zz, q0);
        z -= 8.0 * (z * 0.125).floor();
        n = z as i32;
        z -= f64::from(n);
        ih = 0;
        if q0 > 0 {
            let i = iq[(jz - 1) as usize] >> (24 - q0);
            n += i;
            iq[(jz - 1) as usize] -= i << (24 - q0);
            ih = iq[(jz - 1) as usize] >> (23 - q0);
        } else if q0 == 0 {
            ih = iq[(jz - 1) as usize] >> 23;
        } else if z >= 0.5 {
            ih = 2;
        }
        if ih > 0 {
            n += 1;
            let carry = complement(&mut iq, jz, q0);
            if ih == 2 {
                z = 1.0 - z;
                if carry != 0 {
                    z -= libm::scalbn(1.0, q0);
                }
            }
        }
        if z != 0.0 || (JK..jz).any(|i| iq[i as usize] != 0) {
            break;
        }
        // Every kept digit cancelled: bring in more digits of 2/pi and go again.
        let mut k = 1;
        while JK >= k && iq[(JK - k) as usize] == 0 {
            k += 1;
        }
        for i in (jz + 1)..=(jz + k) {
            f[(jx + i) as usize] = f64::from(TWO_OVER_PI[(jv + i) as usize]);
            q[i as usize] = digit_product(x, &f, jx, i);
        }
        jz += k;
    }
    if z == 0.0 {
        jz -= 1;
        q0 -= 24;
        while iq[jz as usize] == 0 {
            jz -= 1;
            q0 -= 24;
        }
    } else {
        z = libm::scalbn(z, -q0);
        if z >= TWO24 {
            let fw = f64::from((TWON24 * z) as i32);
            iq[jz as usize] = (z - TWO24 * fw) as i32;
            jz += 1;
            q0 += 24;
            iq[jz as usize] = fw as i32;
        } else {
            iq[jz as usize] = z as i32;
        }
    }
    let mut fw = libm::scalbn(1.0, q0);
    for i in (0..=jz).rev() {
        q[i as usize] = fw * f64::from(iq[i as usize]);
        fw *= TWON24;
    }
    let mut fq = [0.0; WORK];
    for i in (0..=jz).rev() {
        let mut fw = 0.0;
        let mut k = 0;
        while k <= JK && k <= jz - i {
            fw += PIO2[k as usize] * q[(i + k) as usize];
            k += 1;
        }
        fq[(jz - i) as usize] = fw;
    }
    let mut fw = 0.0;
    for i in (0..=jz).rev() {
        fw += fq[i as usize];
    }
    let y0 = if ih == 0 { fw } else { -fw };
    let mut fw = fq[0] - fw;
    for i in 1..=jz {
        fw += fq[i as usize];
    }
    let y1 = if ih == 0 { fw } else { -fw };
    (n & 7, y0, y1)
}

/** q[i]: the digits of x against the digits of 2/pi, in fdlibm's summation order. */
fn digit_product(x: &[f64], f: &[f64; WORK], jx: i32, i: i32) -> f64 {
    let mut fw = 0.0;
    for j in 0..=jx {
        fw += x[j as usize] * f[(jx + i - j) as usize];
    }
    fw
}

/** 1 - q over the kept digits, when the fraction is above a half; returns the carry. */
fn complement(iq: &mut [i32; WORK], jz: i32, q0: i32) -> i32 {
    let mut carry = 0;
    for digit in iq.iter_mut().take(jz as usize) {
        let j = *digit;
        if carry == 0 {
            if j != 0 {
                carry = 1;
                *digit = 0x100_0000 - j;
            }
        } else {
            *digit = 0xFF_FFFF - j;
        }
    }
    if q0 == 1 {
        iq[(jz - 1) as usize] &= 0x7F_FFFF;
    } else if q0 == 2 {
        iq[(jz - 1) as usize] &= 0x3F_FFFF;
    }
    carry
}

/** The tangent polynomial coefficients, fdlibm's `T[0..13]`. */
const T: [f64; 13] = [
    3.333_333_333_333_340_919_86e-1,
    1.333_333_333_332_012_426_99e-1,
    5.396_825_397_622_605_213_77e-2,
    2.186_948_829_485_954_245_99e-2,
    8.863_239_823_599_300_057_37e-3,
    3.592_079_107_591_312_353_56e-3,
    1.456_209_454_325_290_255_16e-3,
    5.880_412_408_202_640_968_74e-4,
    2.464_631_348_184_699_068_12e-4,
    7.817_944_429_395_570_923_00e-5,
    7.140_724_913_826_081_903_05e-5,
    -1.855_863_748_552_754_566_54e-5,
    2.590_730_518_636_337_128_84e-5,
];
/** pi/4. */
const PIO4: f64 = 7.853_981_633_974_482_789_99e-1;
/** The low part of pi/4. */
const PIO4LO: f64 = 3.061_616_997_868_383_017_93e-17;
/** The high word of 2^-28, below which tan is its argument. */
const TAN_TINY_HIGH: i32 = 0x3E30_0000;
/** The high word of 0.6744, above which tan folds through pi/4. */
const TAN_FOLD_HIGH: i32 = 0x3FE5_9428;

/** tan on [-pi/4, pi/4] from a two-double argument (fdlibm's `__kernel_tan`); `iy` of 1 asks
for tan, of -1 for -1/tan. */
fn kernel_tan(x: f64, y: f64, iy: i32) -> f64 {
    let hx = high(x);
    let ix = hx & ABS_MASK;
    if ix < TAN_TINY_HIGH {
        if ((ix | low(x) as i32) | (iy + 1)) == 0 {
            return 1.0 / x.abs();
        }
        if iy == 1 {
            return x;
        }
        return negative_reciprocal(x + y, x, y);
    }
    let (mut x, mut y) = (x, y);
    if ix >= TAN_FOLD_HIGH {
        if hx < 0 {
            x = -x;
            y = -y;
        }
        let z = PIO4 - x;
        let w = PIO4LO - y;
        x = z + w;
        y = 0.0;
    }
    let z = x * x;
    let w = z * z;
    let r = T[1] + w * (T[3] + w * (T[5] + w * (T[7] + w * (T[9] + w * T[11]))));
    let v = z * (T[2] + w * (T[4] + w * (T[6] + w * (T[8] + w * (T[10] + w * T[12])))));
    let s = z * x;
    let r = y + z * (s * (r + v) + y);
    let r = r + T[0] * s;
    let w = x + r;
    if ix >= TAN_FOLD_HIGH {
        let v = f64::from(iy);
        return f64::from(1 - ((hx >> 30) & 2)) * (v - 2.0 * (x - (w * w / (w + v) - r)));
    }
    if iy == 1 {
        return w;
    }
    negative_reciprocal(w, x, r)
}

/** -1 / (x + r) to full precision, given w = x + r already rounded. */
fn negative_reciprocal(w: f64, x: f64, r: f64) -> f64 {
    let z = with_low(w, 0);
    let v = r - (z - x);
    let a = -1.0 / w;
    let t = with_low(a, 0);
    let s = 1.0 + t * z;
    t + a * (s + t * v)
}

/** Math.tan. */
pub(super) fn tan(x: f64) -> f64 {
    let ix = high(x) & ABS_MASK;
    if ix <= PIO4_HIGH {
        return kernel_tan(x, 0.0, 1);
    }
    if ix >= INF_HIGH {
        return x - x;
    }
    let (n, y0, y1) = rem_pio2(x);
    kernel_tan(y0, y1, 1 - ((n & 1) << 1))
}

/** The exp overflow threshold. */
const EXP_OVERFLOW: f64 = 7.097_827_128_933_839_730_96e2;
/** The exp underflow threshold. */
const EXP_UNDERFLOW: f64 = -7.451_332_191_019_411_084_20e2;
/** The high word of the exp thresholds. */
const EXP_LIMIT_HIGH: i32 = 0x4086_2E42;
/** The high part of ln 2. */
const LN2_HI: f64 = 6.931_471_803_691_238_164_90e-1;
/** The low part of ln 2. */
const LN2_LO: f64 = 1.908_214_929_270_587_700_02e-10;
/** 1 / ln 2. */
const INVLN2: f64 = 1.442_695_040_888_963_387_00;
/** The exp polynomial. */
const EXP_P: [f64; 5] = [
    1.666_666_666_666_660_190_37e-1,
    -2.777_777_777_701_559_338_42e-3,
    6.613_756_321_437_934_361_17e-5,
    -1.653_390_220_546_525_153_90e-6,
    4.138_136_797_057_238_460_39e-8,
];
/** The high word of 0.5 ln 2. */
const HALF_LN2_HIGH: i32 = 0x3FD6_2E42;
/** The high word of 1.5 ln 2. */
const THREE_HALVES_LN2_HIGH: i32 = 0x3FF0_A2B2;
/** The high word of 2^-28, below which exp is 1 + x. */
const EXP_TINY_HIGH: i32 = 0x3E30_0000;
/** 2^1023. */
const TWO1023: f64 = 8.988_465_674_311_579_539e307;
/** 2^-1000. */
const TWOM1000: f64 = 9.332_636_185_032_188_789_90e-302;
/** Below this k the scale is applied in two steps to avoid a subnormal power of two. */
const EXP_SPLIT_K: i32 = -1021;
/** Math.E, which the polynomial misses in the last bit, so V8 returns it directly. */
const E: f64 = std::f64::consts::E;

/** Math.exp. */
pub(super) fn exp(x: f64) -> f64 {
    let hx = high(x);
    let xsb = ((hx >> 31) & 1) as usize;
    let hx = hx & ABS_MASK;
    if hx >= EXP_LIMIT_HIGH {
        if hx >= INF_HIGH {
            if ((hx & MANTISSA_MASK) | low(x) as i32) != 0 {
                return x + x;
            }
            return if xsb == 0 { x } else { 0.0 };
        }
        if x > EXP_OVERFLOW {
            return f64::INFINITY;
        }
        if x < EXP_UNDERFLOW {
            return 0.0;
        }
    }
    if x == 1.0 {
        return E;
    }
    let (hi, lo, k, x) = exp_reduce(x, hx, xsb);
    if hx < EXP_TINY_HIGH {
        return 1.0 + x;
    }
    let t = x * x;
    let [p1, p2, p3, p4, p5] = EXP_P;
    let c = x - t * (p1 + t * (p2 + t * (p3 + t * (p4 + t * p5))));
    if k == 0 {
        return 1.0 - ((x * c) / (c - 2.0) - x);
    }
    let y = 1.0 - ((lo - (x * c) / (2.0 - c)) - hi);
    if k >= EXP_SPLIT_K {
        if k == 1024 {
            return y * 2.0 * TWO1023;
        }
        return y * power_of_two(k);
    }
    y * power_of_two(k + 1000) * TWOM1000
}

/** 2^k for k in the normal range, built from its exponent word. */
fn power_of_two(k: i32) -> f64 {
    from_words((ONE_HIGH as u32).wrapping_add((k as u32) << 20) as i32, 0)
}

/** Reduce x to hi - lo with x = k ln 2 + (hi - lo); returns (hi, lo, k, reduced x). */
fn exp_reduce(x: f64, hx: i32, xsb: usize) -> (f64, f64, i32, f64) {
    if hx <= HALF_LN2_HIGH {
        return (0.0, 0.0, 0, x);
    }
    let sign = if xsb == 0 { 1.0 } else { -1.0 };
    let (hi, lo, k) = if hx < THREE_HALVES_LN2_HIGH {
        (x - sign * LN2_HI, sign * LN2_LO, 1 - 2 * xsb as i32)
    } else {
        let k = (INVLN2 * x + sign * 0.5) as i32;
        let t = f64::from(k);
        (x - t * LN2_HI, t * LN2_LO, k)
    };
    (hi, lo, k, hi - lo)
}

/** The log polynomial, fdlibm's `Lg1..Lg7`. */
const LG: [f64; 7] = [
    6.666_666_666_666_735_130e-1,
    3.999_999_999_940_941_908e-1,
    2.857_142_874_366_239_149e-1,
    2.222_219_843_214_978_396e-1,
    1.818_357_216_161_805_012e-1,
    1.531_383_769_920_937_332e-1,
    1.479_819_860_511_658_591e-1,
];
/** The high word of the smallest normal double. */
const NORMAL_HIGH: i32 = 0x0010_0000;
/** The offset that rounds the mantissa toward sqrt(2) when normalizing. */
const SQRT2_OFFSET: i32 = 0x95F64;
/** The upper split of the log polynomial's argument. */
const LOG_SPLIT_HI: i32 = 0x6147A;
/** The lower split of the log polynomial's argument. */
const LOG_SPLIT_LO: i32 = 0x6B851;

/** Math.log. */
pub(super) fn log(x: f64) -> f64 {
    let Some((x, hx, k)) = log_normalize(x) else {
        return log_special(x);
    };
    let f = x - 1.0;
    let dk = f64::from(k);
    if (MANTISSA_MASK & (2 + hx)) < 3 {
        if f == 0.0 {
            return if k == 0 {
                0.0
            } else {
                dk * LN2_HI + dk * LN2_LO
            };
        }
        let r = f * f * (0.5 - 0.333_333_333_333_333_33 * f);
        return if k == 0 {
            f - r
        } else {
            dk * LN2_HI - ((r - dk * LN2_LO) - f)
        };
    }
    let s = f / (2.0 + f);
    let z = s * s;
    let w = z * z;
    let i = (hx - LOG_SPLIT_HI) | (LOG_SPLIT_LO - hx);
    let r = log_poly(z, w);
    if i > 0 {
        let hfsq = 0.5 * f * f;
        if k == 0 {
            f - (hfsq - s * (hfsq + r))
        } else {
            dk * LN2_HI - ((hfsq - (s * (hfsq + r) + dk * LN2_LO)) - f)
        }
    } else if k == 0 {
        f - s * (f - r)
    } else {
        dk * LN2_HI - ((s * (f - r) - dk * LN2_LO) - f)
    }
}

/** The log polynomial R(z) with w = z^2. */
fn log_poly(z: f64, w: f64) -> f64 {
    let [lg1, lg2, lg3, lg4, lg5, lg6, lg7] = LG;
    let t1 = w * (lg2 + w * (lg4 + w * lg6));
    let t2 = z * (lg1 + w * (lg3 + w * (lg5 + w * lg7)));
    t2 + t1
}

/** x as m in [sqrt(2)/2, sqrt(2)) times 2^k, with m's mantissa word; None for zero, negative,
infinite and NaN arguments. */
fn log_normalize(x: f64) -> Option<(f64, i32, i32)> {
    let (mut hx, lx) = (high(x), low(x));
    let mut k = 0;
    let mut x = x;
    if hx < NORMAL_HIGH {
        if ((hx & ABS_MASK) | lx as i32) == 0 || hx < 0 {
            return None;
        }
        k -= 54;
        x *= TWO54;
        hx = high(x);
    }
    if hx >= INF_HIGH {
        return None;
    }
    k += (hx >> 20) - BIAS;
    let hx = hx & MANTISSA_MASK;
    let i = (hx + SQRT2_OFFSET) & 0x10_0000;
    let x = with_high(x, hx | (i ^ ONE_HIGH));
    Some((x, hx, k + (i >> 20)))
}

/** log of zero, a negative number, infinity or NaN. */
fn log_special(x: f64) -> f64 {
    if x == 0.0 {
        f64::NEG_INFINITY
    } else if x < 0.0 {
        f64::NAN
    } else {
        x + x
    }
}

/** The high part of 1 / ln 2. */
const IVLN2HI: f64 = 1.442_695_040_721_446_275_71;
/** The low part of 1 / ln 2. */
const IVLN2LO: f64 = 1.675_171_316_488_651_183_53e-10;

/** Math.log2. */
pub(super) fn log2(x: f64) -> f64 {
    let Some((x, _, k)) = log_normalize(x) else {
        return log_special(x);
    };
    if x == 1.0 && k == 0 {
        return 0.0;
    }
    let y = f64::from(k);
    let f = x - 1.0;
    let hfsq = 0.5 * f * f;
    let r = k_log1p(f);
    let hi = with_low(f - hfsq, 0);
    let lo = (f - hi) - hfsq + r;
    let val_hi = hi * IVLN2HI;
    let val_lo = (lo + hi) * IVLN2LO + lo * IVLN2HI;
    let w = y + val_hi;
    let val_lo = val_lo + ((y - w) + val_hi);
    val_lo + w
}

/** log(1 + f) - (f - f^2/2) for small f: the polynomial part of the logarithm. */
fn k_log1p(f: f64) -> f64 {
    let s = f / (2.0 + f);
    let z = s * s;
    let w = z * z;
    let hfsq = 0.5 * f * f;
    s * (hfsq + log_poly(z, w))
}

/** 1 / ln 10. */
const IVLN10: f64 = 4.342_944_819_032_518_166_68e-1;
/** The high part of log10(2). */
const LOG10_2HI: f64 = 3.010_299_956_636_117_713_06e-1;
/** The low part of log10(2). */
const LOG10_2LO: f64 = 3.694_239_077_158_930_786_16e-13;

/** Math.log10. */
pub(super) fn log10(x: f64) -> f64 {
    let (mut hx, mut lx) = (high(x), low(x));
    let mut k = 0;
    let mut x = x;
    if hx < NORMAL_HIGH {
        if ((hx & ABS_MASK) | lx as i32) == 0 || hx < 0 {
            return log_special(x);
        }
        k -= 54;
        x *= TWO54;
        hx = high(x);
        lx = low(x);
    }
    if hx >= INF_HIGH {
        return x + x;
    }
    if hx == ONE_HIGH && lx == 0 {
        return 0.0;
    }
    k += (hx >> 20) - BIAS;
    let i = ((k as u32) & 0x8000_0000) >> 31;
    let hx = (hx & MANTISSA_MASK) | ((0x3FF - i as i32) << 20);
    let y = f64::from(k + i as i32);
    let x = from_words(hx, lx);
    let z = y * LOG10_2LO + IVLN10 * log(x);
    z + y * LOG10_2HI
}

/** The cbrt exponent bias for normal arguments. */
const CBRT_B1: u32 = 715_094_163;
/** The cbrt exponent bias for subnormal arguments scaled by 2^54. */
const CBRT_B2: u32 = 696_219_795;
/** The polynomial approximating 1/cbrt near 1. */
const CBRT_P: [f64; 5] = [
    1.875_951_824_271_770_096_43,
    -1.884_979_795_433_771_698_75,
    1.621_429_720_105_354_466_140,
    -0.758_397_934_778_766_047_437,
    0.145_996_192_886_612_446_982,
];
/** The high word of 2^54. */
const TWO54_HIGH: i32 = 0x4350_0000;

/** Math.cbrt. */
pub(super) fn cbrt(x: f64) -> f64 {
    let hx = high(x);
    let lx = low(x);
    let sign = (hx as u32) & 0x8000_0000;
    let hx = (hx as u32) ^ sign;
    if hx as i32 >= INF_HIGH {
        return x + x;
    }
    let t = if (hx as i32) < NORMAL_HIGH {
        if (hx | lx) == 0 {
            return x;
        }
        let t = from_words(TWO54_HIGH, 0) * x;
        let hi = (high(t) as u32) & 0x7FFF_FFFF;
        from_words((sign | (hi / 3 + CBRT_B2)) as i32, 0)
    } else {
        from_words((sign | (hx / 3 + CBRT_B1)) as i32, 0)
    };
    let [p0, p1, p2, p3, p4] = CBRT_P;
    let r = (t * t) * (t / x);
    let t = t * ((p0 + r * (p1 + r * p2)) + ((r * r) * r) * (p3 + r * p4));
    let t = f64::from_bits((t.to_bits() + 0x8000_0000) & 0xFFFF_FFFF_C000_0000);
    let s = t * t;
    let r = x / s;
    let w = t + t;
    let r = (r - t) / (w + r);
    t + t * r
}

/** The high word of 22, past which tanh is one. */
const TANH_ONE_HIGH: i32 = 0x4036_0000;
/** The high word of 2^-28, below which tanh is its argument. */
const TANH_TINY_HIGH: i32 = 0x3E30_0000;

/** Math.tanh. */
pub(super) fn tanh(x: f64) -> f64 {
    let jx = high(x);
    let ix = jx & ABS_MASK;
    if ix >= INF_HIGH {
        return if jx >= 0 {
            1.0 / x + 1.0
        } else {
            1.0 / x - 1.0
        };
    }
    let z = if ix < TANH_ONE_HIGH {
        if ix < TANH_TINY_HIGH {
            return x;
        }
        if ix >= ONE_HIGH {
            let t = libm::expm1(2.0 * x.abs());
            1.0 - 2.0 / (t + 2.0)
        } else {
            let t = libm::expm1(-2.0 * x.abs());
            -t / (t + 2.0)
        }
    } else {
        1.0
    };
    if jx >= 0 { z } else { -z }
}
