/** Maps a transform type to the 1-D transform of each pass and to the flips applied when the residual is added (spec 7.12.3 and 7.13.3). The two-letter names of the transform types are column then row, so ADST_DCT is an ADST down the columns and a DCT along the rows. */
/** The 1-D transform a pass applies. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Dct,
    Adst,
    Identity,
}

/** The passes of a transform type and whether its residual is flipped vertically or horizontally. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TxKinds {
    pub(crate) row: Kind,
    pub(crate) col: Kind,
    pub(crate) flip_ud: bool,
    pub(crate) flip_lr: bool,
}

/** The row kind of each transform type, indexed by the type (DCT_DCT is 0, H_FLIPADST is 15). */
const ROW: [Kind; 16] = {
    use Kind::{Adst as A, Dct as D, Identity as I};
    [D, D, A, A, D, A, A, A, A, I, I, D, I, A, I, A]
};

/** The column kind of each transform type, indexed by the type. */
const COL: [Kind; 16] = {
    use Kind::{Adst as A, Dct as D, Identity as I};
    [D, A, D, A, A, D, A, A, A, I, D, I, A, I, A, I]
};

/** Whether each transform type flips the residual vertically (FLIPADST in the column pass). */
const FLIP_UD: [bool; 16] = [
    false, false, false, false, true, false, true, false, true, false, false, false, false, false,
    true, false,
];

/** Whether each transform type flips the residual horizontally (FLIPADST in the row pass). */
const FLIP_LR: [bool; 16] = [
    false, false, false, false, false, true, true, true, false, false, false, false, false, false,
    false, true,
];

/** The passes and flips of a transform type. The tables are indexed by the type, which is always below 16 for an intra block. */
pub(crate) fn tx_kinds(tx_type: u8) -> TxKinds {
    let t = usize::from(tx_type) & 15;
    TxKinds {
        row: ROW[t],
        col: COL[t],
        flip_ud: FLIP_UD[t],
        flip_lr: FLIP_LR[t],
    }
}
