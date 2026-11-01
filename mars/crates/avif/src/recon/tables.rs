/** Constant tables of the reconstruction stage, copied from the AV1 specification (sections 7.11.2, 7.11.3 and 7.13.2) and flattened row-major. */
/** Smooth-prediction weights for a 4-sample edge (spec table Sm_Weights_Tx_4x4). */
pub(crate) static SM_WEIGHTS_TX_4X4: [u8; 4] = [255, 149, 85, 64];

/** Smooth-prediction weights for an 8-sample edge (spec table Sm_Weights_Tx_8x8). */
pub(crate) static SM_WEIGHTS_TX_8X8: [u8; 8] = [255, 197, 146, 105, 73, 50, 37, 32];

/** Smooth-prediction weights for a 16-sample edge (spec table Sm_Weights_Tx_16x16). */
pub(crate) static SM_WEIGHTS_TX_16X16: [u8; 16] = [
    255, 225, 196, 170, 145, 123, 102, 84, 68, 54, 43, 33, 26, 20, 17, 16,
];

/** Smooth-prediction weights for a 32-sample edge (spec table Sm_Weights_Tx_32x32). */
pub(crate) static SM_WEIGHTS_TX_32X32: [u8; 32] = [
    255, 240, 225, 210, 196, 182, 169, 157, 145, 133, 122, 111, 101, 92, 83, 74, 66, 59, 52, 45,
    39, 34, 29, 25, 21, 17, 14, 12, 10, 9, 8, 8,
];

/** Smooth-prediction weights for a 64-sample edge (spec table Sm_Weights_Tx_64x64). */
pub(crate) static SM_WEIGHTS_TX_64X64: [u8; 64] = [
    255, 248, 240, 233, 225, 218, 210, 203, 196, 189, 182, 176, 169, 163, 156, 150, 144, 138, 133,
    127, 121, 116, 111, 106, 101, 96, 91, 86, 82, 77, 73, 69, 65, 61, 57, 54, 50, 47, 44, 41, 38,
    35, 32, 29, 27, 25, 22, 20, 18, 16, 15, 13, 12, 10, 9, 8, 7, 6, 6, 5, 5, 4, 4, 4,
];

/** Nominal prediction angle of each intra mode, in degrees (spec Mode_To_Angle). */
pub(crate) static MODE_TO_ANGLE: [i32; 13] = [0, 90, 180, 45, 135, 113, 157, 203, 67, 0, 0, 0, 0];

/** Directional-prediction step per degree, indexed by angle (spec Dr_Intra_Derivative). */
pub(crate) static DR_INTRA_DERIVATIVE: [i32; 90] = [
    0, 0, 0, 1023, 0, 0, 547, 0, 0, 372, 0, 0, 0, 0, 273, 0, 0, 215, 0, 0, 178, 0, 0, 151, 0, 0,
    132, 0, 0, 116, 0, 0, 102, 0, 0, 0, 90, 0, 0, 80, 0, 0, 71, 0, 0, 64, 0, 0, 57, 0, 0, 51, 0, 0,
    45, 0, 0, 0, 40, 0, 0, 35, 0, 0, 31, 0, 0, 27, 0, 0, 23, 0, 0, 19, 0, 0, 15, 0, 0, 0, 0, 11, 0,
    0, 7, 0, 0, 3, 0, 0,
];

/** Recursive intra filter taps, [mode][output position][input tap] flattened (spec Intra_Filter_Taps). */
pub(crate) static INTRA_FILTER_TAPS: [i32; 280] = [
    -6, 10, 0, 0, 0, 12, 0, -5, 2, 10, 0, 0, 9, 0, -3, 1, 1, 10, 0, 7, 0, -3, 1, 1, 2, 10, 5, 0,
    -4, 6, 0, 0, 0, 2, 12, -3, 2, 6, 0, 0, 2, 9, -3, 2, 2, 6, 0, 2, 7, -3, 1, 2, 2, 6, 3, 5, -10,
    16, 0, 0, 0, 10, 0, -6, 0, 16, 0, 0, 6, 0, -4, 0, 0, 16, 0, 4, 0, -2, 0, 0, 0, 16, 2, 0, -10,
    16, 0, 0, 0, 0, 10, -6, 0, 16, 0, 0, 0, 6, -4, 0, 0, 16, 0, 0, 4, -2, 0, 0, 0, 16, 0, 2, -8, 8,
    0, 0, 0, 16, 0, -8, 0, 8, 0, 0, 16, 0, -8, 0, 0, 8, 0, 16, 0, -8, 0, 0, 0, 8, 16, 0, -4, 4, 0,
    0, 0, 0, 16, -4, 0, 4, 0, 0, 0, 16, -4, 0, 0, 4, 0, 0, 16, -4, 0, 0, 0, 4, 0, 16, -2, 8, 0, 0,
    0, 10, 0, -1, 3, 8, 0, 0, 6, 0, -1, 2, 3, 8, 0, 4, 0, 0, 1, 2, 3, 8, 2, 0, -1, 4, 0, 0, 0, 3,
    10, -1, 3, 4, 0, 0, 4, 6, -1, 2, 3, 4, 0, 4, 4, -1, 2, 2, 3, 4, 3, 3, -12, 14, 0, 0, 0, 14, 0,
    -10, 0, 14, 0, 0, 12, 0, -9, 0, 0, 14, 0, 11, 0, -8, 0, 0, 0, 14, 10, 0, -10, 12, 0, 0, 0, 0,
    14, -9, 1, 12, 0, 0, 0, 12, -8, 0, 0, 12, 0, 1, 11, -7, 0, 0, 1, 12, 1, 9,
];

/** 4096 * cos(i * pi / 128) for i in 0..=64 (spec Cos128_Lookup). */
pub(crate) static COS128_LOOKUP: [i32; 65] = [
    4096, 4095, 4091, 4085, 4076, 4065, 4052, 4036, 4017, 3996, 3973, 3948, 3920, 3889, 3857, 3822,
    3784, 3745, 3703, 3659, 3612, 3564, 3513, 3461, 3406, 3349, 3290, 3229, 3166, 3102, 3035, 2967,
    2896, 2824, 2751, 2675, 2598, 2520, 2440, 2359, 2276, 2191, 2106, 2019, 1931, 1842, 1751, 1660,
    1567, 1474, 1380, 1285, 1189, 1092, 995, 897, 799, 700, 601, 501, 401, 301, 201, 101, 0,
];

/** Edge smoothing kernels, [strength - 1][tap] flattened (spec Intra_Edge_Kernel). */
pub(crate) static INTRA_EDGE_KERNEL: [i32; 15] = [0, 4, 8, 4, 0, 0, 5, 6, 5, 0, 2, 4, 4, 4, 2];

/** Samples of the transform row shift per transform size (spec Transform_Row_Shift). */
pub(crate) static TRANSFORM_ROW_SHIFT: [u8; 19] =
    [0, 1, 2, 2, 2, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2];
