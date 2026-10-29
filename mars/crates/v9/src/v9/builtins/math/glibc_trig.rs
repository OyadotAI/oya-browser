/*! Math.sin and Math.cos as Chrome computes them: V8 is built with `v8_use_libm_trig_functions`
and takes both from the copy of glibc's `sysdeps/ieee754/dbl-64/s_sin.c` it bundles, not from
fdlibm. That routine combines a short Taylor series with a table of sines and cosines at 128ths,
and reduces large arguments with `branred.c`; this is a port of both, step for step, so the
last bit matches. The constants are the exact bit patterns from `usncs.h` and `branred.h`. */
#![allow(clippy::eq_op)]
use super::sincostab;

/** -1/3!, the first Taylor coefficient of sin. */
const S1: f64 = f64::from_bits(0xBFC5_5555_5555_5555);
/** 1/5!. */
const S2: f64 = f64::from_bits(0x3F81_1111_1111_0ECE);
/** -1/7!. */
const S3: f64 = f64::from_bits(0xBF2A_01A0_19DB_08B8);
/** 1/9!. */
const S4: f64 = f64::from_bits(0x3EC7_1DE2_7B9A_7ED9);
/** -1/11!. */
const S5: f64 = f64::from_bits(0xBE5A_DDFF_C2FC_DF59);
/** The sin coefficient at x^3 of the table correction. */
const SN3: f64 = f64::from_bits(0xBFC5_5555_5555_5515);
/** The sin coefficient at x^5 of the table correction. */
const SN5: f64 = f64::from_bits(0x3F81_1110_E829_872F);
/** The cos coefficient at x^2 of the table correction. */
const CS2: f64 = f64::from_bits(0x3FE0_0000_0000_0000);
/** The cos coefficient at x^4 of the table correction. */
const CS4: f64 = f64::from_bits(0xBFA5_5555_5555_5535);
/** The cos coefficient at x^6 of the table correction. */
const CS6: f64 = f64::from_bits(0x3F56_C16B_EDD9_E239);
/** 1.5 * 2^45: adding it rounds |x| to a multiple of 1/128, the table's spacing. */
const BIG: f64 = f64::from_bits(0x42C8_0000_0000_0000);
/** pi/2. */
const HP0: f64 = f64::from_bits(0x3FF9_21FB_5444_2D18);
/** The low part of pi/2. */
const HP1: f64 = f64::from_bits(0x3C91_A626_3314_5C07);
/** pi/2 rounded to 28 bits, for the medium reduction. */
const MP1: f64 = f64::from_bits(0x3FF9_21FB_5800_0000);
/** The next 28 bits of pi/2. */
const MP2: f64 = f64::from_bits(0xBE4D_DE97_3C00_0000);
/** The third piece of pi/2. */
const PP3: f64 = f64::from_bits(0xBC8C_B3B3_9800_0000);
/** The fourth piece of pi/2. */
const PP4: f64 = f64::from_bits(0xBACD_747F_23E3_2ED7);
/** 2/pi. */
const HPINV: f64 = f64::from_bits(0x3FE4_5F30_6DC9_C883);
/** 1.5 * 2^52: adding it rounds to an integer in the low word. */
const TOINT: f64 = f64::from_bits(0x4338_0000_0000_0000);
/** The high word of 2^-26, below which sin is its argument. */
const SIN_TINY_HIGH: i32 = 0x3e50_0000;
/** The high word of 2^-27, below which cos is one. */
const COS_TINY_HIGH: i32 = 0x3e40_0000;
/** The high word of 0.855469, the table's reach. */
const TABLE_HIGH: i32 = 0x3feb_6000;
/** The high word of 2.426265, up to which pi/2 - |x| is in the table's reach. */
const FOLD_HIGH: i32 = 0x4003_68fd;
/** The high word of 105414350, up to which the medium reduction is exact enough. */
const MEDIUM_HIGH: i32 = 0x4199_21FB;
/** The high word of infinity and NaN. */
const INF_HIGH: i32 = 0x7ff0_0000;
/** Below this |x| sin takes the Taylor series alone. */
const TAYLOR_LIMIT: f64 = 0.126;

/** The upper 32 bits of a double, signed. */
fn high(x: f64) -> i32 {
    (x.to_bits() >> 32) as i32
}

/** The lower 32 bits of a double, signed, as glibc's `mynumber` reads them. */
fn low(x: f64) -> i32 {
    x.to_bits() as u32 as i32
}

/** Math.sin. */
pub(super) fn sin(x: f64) -> f64 {
    let k = high(x) & 0x7fff_ffff;
    if k < SIN_TINY_HIGH {
        x
    } else if k < TABLE_HIGH {
        do_sin(x, 0.0)
    } else if k < FOLD_HIGH {
        do_cos(HP0 - x.abs(), HP1).copysign(x)
    } else if k < MEDIUM_HIGH {
        let (n, a, da) = reduce_sincos(x);
        do_sincos(a, da, n)
    } else if k < INF_HIGH {
        let (n, a, da) = branred(x);
        do_sincos(a, da, n)
    } else {
        x / x
    }
}

/** Math.cos. */
pub(super) fn cos(x: f64) -> f64 {
    let k = high(x) & 0x7fff_ffff;
    if k < COS_TINY_HIGH {
        1.0
    } else if k < TABLE_HIGH {
        do_cos(x, 0.0)
    } else if k < FOLD_HIGH {
        let y = HP0 - x.abs();
        let a = y + HP1;
        do_sin(a, (y - a) + HP1)
    } else if k < MEDIUM_HIGH {
        let (n, a, da) = reduce_sincos(x);
        do_sincos(a, da, n + 1)
    } else if k < INF_HIGH {
        let (n, a, da) = branred(x);
        do_sincos(a, da, n + 1)
    } else {
        x / x
    }
}

/** The table row for |x| rounded to 128ths: sine, its low part, cosine, its low part. */
fn lookup(u: f64) -> (f64, f64, f64, f64) {
    let k = (low(u) << 2) as usize;
    let entry = |offset| sincostab::entry(k + offset);
    (entry(0), entry(1), entry(2), entry(3))
}

/** sin(x + dx) for |x| up to 0.855 through the table, or the series when x is small. */
fn do_sin(x: f64, dx: f64) -> f64 {
    let xold = x;
    if x.abs() < TAYLOR_LIMIT {
        return taylor_sin(x * x, x, dx);
    }
    let dx = if x <= 0.0 { -dx } else { dx };
    let u = BIG + x.abs();
    let x = x.abs() - (u - BIG);
    let xx = x * x;
    let s = x + (dx + x * xx * (SN3 + xx * SN5));
    let c = x * dx + xx * (CS2 + xx * (CS4 + xx * CS6));
    let (sn, ssn, cs, ccs) = lookup(u);
    let cor = (ssn + s * ccs - sn * c) + cs * s;
    (sn + cor).copysign(xold)
}

/** cos(x + dx) for |x| up to 0.855 through the table. */
fn do_cos(x: f64, dx: f64) -> f64 {
    let dx = if x < 0.0 { -dx } else { dx };
    let u = BIG + x.abs();
    let x = x.abs() - (u - BIG) + dx;
    let xx = x * x;
    let s = x + x * xx * (SN3 + xx * SN5);
    let c = xx * (CS2 + xx * (CS4 + xx * CS6));
    let (sn, ssn, cs, ccs) = lookup(u);
    let cor = (ccs - s * ssn - cs * c) - sn * s;
    cs + cor
}

/** The Taylor series for sin(x + dx) with xx = x^2. */
fn taylor_sin(xx: f64, x: f64, dx: f64) -> f64 {
    let polynomial = (((S5 * xx + S4) * xx + S3) * xx + S2) * xx + S1;
    let t = (polynomial * x - 0.5 * dx) * xx + dx;
    x + t
}

/** Reduce x by multiples of pi/2 to a + da and the quadrant, exact to 136 bits. */
fn reduce_sincos(x: f64) -> (i32, f64, f64) {
    let t = x * HPINV + TOINT;
    let xn = t - TOINT;
    let y = (x - xn * MP1) - xn * MP2;
    let n = low(t) & 3;
    let t1 = xn * PP3;
    let t2 = y - t1;
    let mut db = (y - t2) - t1;
    let t1 = xn * PP4;
    let b = t2 - t1;
    db += (t2 - b) - t1;
    (n, b, db)
}

/** sin or cos of the reduced argument by quadrant. */
fn do_sincos(a: f64, da: f64, n: i32) -> f64 {
    let value = if n & 1 != 0 {
        do_cos(a, da)
    } else {
        do_sin(a, da)
    };
    if n & 2 != 0 { -value } else { value }
}

/** 2/pi in base 2^24, for the huge-argument reduction. */
const TOVERP: [f64; 75] = [
    10680707.0, 7228996.0, 1387004.0, 2578385.0, 16069853.0, 12639074.0, 9804092.0, 4427841.0,
    16666979.0, 11263675.0, 12935607.0, 2387514.0, 4345298.0, 14681673.0, 3074569.0, 13734428.0,
    16653803.0, 1880361.0, 10960616.0, 8533493.0, 3062596.0, 8710556.0, 7349940.0, 6258241.0,
    3772886.0, 3769171.0, 3798172.0, 8675211.0, 12450088.0, 3874808.0, 9961438.0, 366607.0,
    15675153.0, 9132554.0, 7151469.0, 3571407.0, 2607881.0, 12013382.0, 4155038.0, 6285869.0,
    7677882.0, 13102053.0, 15825725.0, 473591.0, 9065106.0, 15363067.0, 6271263.0, 9264392.0,
    5636912.0, 4652155.0, 7056368.0, 13614112.0, 10155062.0, 1944035.0, 9527646.0, 15080200.0,
    6658437.0, 6231200.0, 6832269.0, 16767104.0, 5075751.0, 3212806.0, 1398474.0, 7579849.0,
    6349435.0, 12618859.0, 4703257.0, 12806093.0, 14477321.0, 2786137.0, 12875403.0, 9837734.0,
    14528324.0, 13719321.0, 343717.0,
];
/** 2^27 + 1, which splits a double into two halves by multiplication. */
const SPLIT: f64 = 134_217_729.0;
/** The high word of 2^576. */
const T576_HIGH: u32 = 0x63f0_0000;
/** 2^-600. */
const TM600: f64 = f64::from_bits(0x1a70_0000_0000_0000);
/** 2^-24. */
const TM24: f64 = f64::from_bits(0x3e70_0000_0000_0000);
/** 1.5 * 2^52, which rounds to an integer. */
const ROUND: f64 = f64::from_bits(0x4338_0000_0000_0000);
/** 1.5 * 2^54, which rounds to a multiple of four. */
const ROUND4: f64 = f64::from_bits(0x4358_0000_0000_0000);
/** pi/2 to 28 bits, as branred spells it. */
const BR_MP1: f64 = f64::from_bits(0x3FF9_21FB_5800_0000);
/** The rest of pi/2 to 56 bits, as branred spells it. */
const BR_MP2: f64 = f64::from_bits(0xBE4D_DE97_4000_0000);
/** How many 24-bit digits each half of x is multiplied against. */
const DIGITS: usize = 6;
/** The exponent offset that picks the first useful digit of 2/pi. */
const DIGIT_OFFSET: i32 = 450;
/** Bits per digit of 2/pi. */
const DIGIT_BITS: i32 = 24;

/** glibc's `__branred`: reduce a huge |x| by multiples of pi/2 with 2/pi in 24-bit digits,
returning the quadrant and the remainder as a two-double value. */
fn branred(x: f64) -> (i32, f64, f64) {
    let x = x * TM600;
    let t = x * SPLIT;
    let x1 = t - (t - x);
    let x2 = x - x1;
    let (b1, bb1, sum1) = half(x1);
    let (b2, bb2, sum2) = half(x2);
    let mut sum = sum1 + sum2;
    let mut b = b1 + b2;
    let bb = if b1.abs() > b2.abs() {
        (b1 - b) + b2
    } else {
        (b2 - b) + b1
    };
    if b > 0.5 {
        b -= 1.0;
        sum += 1.0;
    } else if b < -0.5 {
        b += 1.0;
        sum -= 1.0;
    }
    let s = b + (bb + bb1 + bb2);
    let t = ((b - s) + bb) + (bb1 + bb2);
    let b = s * SPLIT;
    let t1 = b - (b - s);
    let t2 = s - t1;
    let b = s * HP0;
    let bb = (((t1 * BR_MP1 - b) + t1 * BR_MP2) + t2 * BR_MP1) + (t2 * BR_MP2 + s * HP1 + t * HP0);
    let s = b + bb;
    let t = (b - s) + bb;
    ((sum as i32) & 3, s, t)
}

/** One half of x times 2/pi: the fraction as b + bb and the integer part modulo four. */
fn half(x: f64) -> (f64, f64, f64) {
    let k = (((high(x) >> 20) & 2047) - DIGIT_OFFSET) / DIGIT_BITS;
    let k = k.max(0) as usize;
    let mut gor =
        f64::from_bits(u64::from(T576_HIGH - ((k as u32 * DIGIT_BITS as u32) << 20)) << 32);
    let mut r = [0.0; DIGITS];
    for (i, digit) in r.iter_mut().enumerate() {
        *digit = x * TOVERP[k + i] * gor;
        gor *= TM24;
    }
    let mut sum = 0.0;
    for digit in r.iter_mut().take(3) {
        let s = (*digit + ROUND) - ROUND;
        sum += s;
        *digit -= s;
    }
    let mut t = 0.0;
    for digit in r.iter().rev() {
        t += digit;
    }
    let bb = (((((r[0] - t) + r[1]) + r[2]) + r[3]) + r[4]) + r[5];
    let s = (t + ROUND) - ROUND;
    sum += s;
    let t = t - s;
    let b = t + bb;
    let bb = (t - b) + bb;
    let s = (sum + ROUND4) - ROUND4;
    (b, bb, sum - s)
}
