use super::coeff_ctx::coeff_base_eob_ctx;
/** Tile decoder tests: the fixtures' tiles must read to the end of their symbol streams with the padding in place, malformed tile bytes must fail rather than panic, the decoded modes and transforms must look like a photograph, and the helper tables and rules must hold on their own. */
use super::constants::{
    BLOCK_64X64, DCT_DCT, H_DCT, IDTX, MAX_REF_MV_STACK_SIZE, PARTITION_SPLIT, TX_4X4, TX_16X16,
    TX_16X64, TX_32X32, TX_64X16, TX_64X64, TX_SIZES_ALL, V_DCT,
};
use super::decode_frame;
use super::decoder::TileBounds;
use super::group::split_tiles;
use super::intra_mode::{coded_segment, neg_deinterleave};
use super::intrabc_valid;
use super::lr::inverse_recenter;
use super::mv_stack;
use super::palette::{ceil_log2, merge_cache};
use super::scan::get_scan;
use super::state::FrameState;
use super::tables::{
    ADJUSTED_TX_SIZE, MAX_TX_SIZE_RECT, PARTITION_SUBSIZE, SPLIT_TX_SIZE, TX_HEIGHT,
    TX_SIZE_SQR_UP, TX_WIDTH,
};
use super::tx_tree;
use crate::container;
use crate::error::{Error, Result};
use crate::frame::{FrameHeader, Parsed};
use crate::sequence::Sequence;
use crate::stream;
use crate::test_support::{AIRBNB, jpeg_luma, psnr};

/** Spec indices the table checks compare against, named here because the decoder itself does not name them. */
const BLOCK_32X32: usize = 9;
const TX_8X8: usize = 1;
const TX_32X16: usize = 10;

/** Official AVIF test vectors: a clap-cropped 8-bit 4:2:0 file, 4:4:4, monochrome, and a still picture. */
const CHIMERA_8BIT: &[u8] = include_bytes!("../../tests/fixtures/chimera-8bit.avif");
const MEXICO_444: &[u8] = include_bytes!("../../tests/fixtures/mexico-yuv444.avif");
const MONOCHROME: &[u8] = include_bytes!("../../tests/fixtures/monochrome.avif");
const STILL: &[u8] = include_bytes!("../../tests/fixtures/still-picture.avif");

/** Every fixture's tiles, so their symbol streams can be followed to the end. The monochrome and still-picture fixtures enable screen content and intra block copy. */
const SYNCED: [(&str, &[u8]); 5] = [
    ("airbnb-240", AIRBNB),
    ("chimera-8bit", CHIMERA_8BIT),
    ("mexico-yuv444", MEXICO_444),
    ("monochrome", MONOCHROME),
    ("still-picture", STILL),
];

/** Run `f` on a fixture's sequence header, frame header and tile group payloads. */
fn with_frame<R>(
    bytes: &[u8],
    f: impl FnOnce(&Sequence, &FrameHeader, &[&[u8]]) -> R,
) -> Result<R> {
    let picture = container::open(bytes)?;
    let payload = picture.payload(picture.primary)?;
    let headers = stream::parse_headers(&payload, &picture.av1c)?;
    let Parsed::Frame(frame) = &headers.frame else {
        return Err(Error::Unsupported("show-existing frame"));
    };
    let hdr: &FrameHeader = frame;
    let groups = stream::tile_groups(&payload, hdr)?;
    Ok(f(&headers.seq, hdr, &groups))
}

/** Decode every tile of a fixture's frame. */
fn decode_fixture(bytes: &[u8]) -> Result<FrameState> {
    with_frame(bytes, decode_frame)?
}

/** Each tile of every fixture, intra block copy included, reads to the end of its symbol stream, and the padding after the trailing bit is zero. A pass proves the block syntax consumed exactly the bits the encoder wrote. */
#[test]
fn every_tile_without_intra_block_copy_ends_in_step() {
    for (name, bytes) in SYNCED {
        if let Err(error) = decode_fixture(bytes) {
            panic!("{name}: tile syntax out of step: {error}");
        }
    }
}

/** A tile whose bytes stop early runs out of symbols, and the padding check refuses it. */
#[test]
fn a_tile_cut_short_fails_the_padding_check() {
    let result = with_frame(AIRBNB, |seq, hdr, groups| {
        let cut = &groups[0][..groups[0].len() / 2];
        decode_frame(seq, hdr, &[cut])
    });
    assert!(result.unwrap().is_err());
}

/** Bytes changed inside a tile may decode to anything, but must never panic: each corruption is decoded and its result ignored. */
#[test]
fn corrupted_tile_bytes_never_panic() {
    with_frame(AIRBNB, |seq, hdr, groups| {
        let original = groups[0];
        let stride = (original.len() / 16).max(1);
        for at in (0..original.len()).step_by(stride) {
            let mut bytes = original.to_vec();
            bytes[at] ^= 0xA5;
            let _ = decode_frame(seq, hdr, &[&bytes]);
        }
    })
    .unwrap();
}

/** The airbnb photograph decodes to a mix of block sizes and modes, every 4x4 unit is covered by a block, and transform blocks carry coefficients. */
#[test]
fn the_airbnb_modes_look_like_a_photograph() {
    let (fs, rows, cols) = with_frame(AIRBNB, |seq, hdr, groups| {
        let fs = decode_frame(seq, hdr, groups).unwrap();
        (fs, hdr.size.mi_rows as usize, hdr.size.mi_cols as usize)
    })
    .unwrap();
    let mut y_modes = [0usize; 13];
    let mut sizes = [0usize; 22];
    for r in 0..rows {
        for c in 0..cols {
            let info = fs.mi.at(r, c);
            y_modes[usize::from(info.y_mode)] += 1;
            sizes[usize::from(info.mi_size)] += 1;
        }
    }
    let total: usize = sizes.iter().sum();
    assert_eq!(total, rows * cols);
    let dominant = y_modes[0] + y_modes[9];
    assert!(
        dominant * 2 > total,
        "DC and smooth should dominate a photograph: {y_modes:?}"
    );
    assert!(
        y_modes.iter().filter(|&&n| n > 0).count() >= 4,
        "{y_modes:?}"
    );
    assert!(
        fs.planes[0].row(0).iter().any(|&v| v != 0),
        "luma was not reconstructed"
    );
}

/** Every scan order is a permutation of its block's positions, so each coefficient position is read exactly once. The one-dimensional types only occur in transforms up to 16 samples, as the transform sets allow. */
#[test]
fn every_scan_is_a_permutation_of_its_transform() {
    for tx in 0..TX_SIZES_ALL {
        let one_dimensional = [V_DCT, H_DCT];
        let allowed: &[u8] = if TX_SIZE_SQR_UP[tx] as usize <= TX_16X16 {
            &one_dimensional
        } else {
            &[]
        };
        for &tx_type in [DCT_DCT, IDTX].iter().chain(allowed) {
            let scan = get_scan(tx, tx_type);
            let adj = ADJUSTED_TX_SIZE[tx] as usize;
            let area = (TX_WIDTH[adj] as usize).min(32) * (TX_HEIGHT[adj] as usize).min(32);
            assert_eq!(scan.len(), area, "tx {tx} type {tx_type}");
            let mut seen = vec![false; area];
            for &pos in scan {
                assert!(!seen[pos as usize], "tx {tx} repeats position {pos}");
                seen[pos as usize] = true;
            }
        }
    }
}

/** The generated partition and transform tables carry the spec's known entries. */
#[test]
fn the_generated_tables_hold_the_spec_entries() {
    assert_eq!(
        PARTITION_SUBSIZE[PARTITION_SPLIT][BLOCK_64X64] as usize,
        BLOCK_32X32
    );
    assert_eq!(MAX_TX_SIZE_RECT[20] as usize, TX_16X64);
    assert_eq!(SPLIT_TX_SIZE[TX_64X64] as usize, TX_32X32);
    assert_eq!(SPLIT_TX_SIZE[TX_16X16] as usize, TX_8X8);
    assert_eq!(SPLIT_TX_SIZE[TX_64X16] as usize, TX_32X16);
    assert_eq!(SPLIT_TX_SIZE[TX_4X4] as usize, TX_4X4);
}

/** The eob context changes at one eighth and one quarter of the coded area. */
#[test]
fn the_eob_context_splits_the_area_by_eighths_and_quarters() {
    // A 4x4 transform has 16 positions: an eighth is two, a quarter is four.
    assert_eq!(coeff_base_eob_ctx(0, TX_4X4), 0);
    assert_eq!(coeff_base_eob_ctx(1, TX_4X4), 1);
    assert_eq!(coeff_base_eob_ctx(2, TX_4X4), 1);
    assert_eq!(coeff_base_eob_ctx(3, TX_4X4), 2);
    assert_eq!(coeff_base_eob_ctx(4, TX_4X4), 2);
    assert_eq!(coeff_base_eob_ctx(5, TX_4X4), 3);
}

/** The palette cache is the sorted union of the two neighbour palettes, with repeats removed. */
#[test]
fn the_palette_cache_merges_the_neighbours_without_repeats() {
    assert_eq!(merge_cache(&[1, 3, 5], &[2, 3, 6]), vec![1, 2, 3, 5, 6]);
    assert_eq!(merge_cache(&[], &[4, 4]), vec![4]);
    assert_eq!(merge_cache(&[7], &[]), vec![7]);
}

/** CeilLog2 is zero below two and rounds up above it. */
#[test]
fn ceil_log2_rounds_up_from_two() {
    assert_eq!(ceil_log2(0), 0);
    assert_eq!(ceil_log2(1), 0);
    assert_eq!(ceil_log2(2), 1);
    assert_eq!(ceil_log2(3), 2);
    assert_eq!(ceil_log2(4), 2);
    assert_eq!(ceil_log2(5), 3);
}

/** The sub-exponential re-centring keeps small codes next to the reference and passes large codes through. */
#[test]
fn inverse_recenter_keeps_small_codes_near_the_reference() {
    assert_eq!(inverse_recenter(5, 0), 5);
    assert_eq!(inverse_recenter(5, 1), 4);
    assert_eq!(inverse_recenter(5, 2), 6);
    assert_eq!(inverse_recenter(5, 11), 11);
}

/** A segment difference maps back around its prediction, and a zero prediction takes the difference as it is. */
#[test]
fn neg_deinterleave_maps_differences_around_the_prediction() {
    assert_eq!(neg_deinterleave(3, 0, 8), 3);
    assert_eq!(neg_deinterleave(0, 4, 8), 4);
    assert_eq!(neg_deinterleave(1, 4, 8), 5);
    assert_eq!(neg_deinterleave(2, 4, 8), 3);
    assert_eq!(neg_deinterleave(0, 7, 8), 7);
}

/** A coded segment difference from hostile bits can name an id past the active segments; it is clipped to the last one, never past the eight segment slots. */
#[test]
fn a_coded_segment_is_clipped_to_the_last_active_segment() {
    assert_eq!(coded_segment(7, 0, 2), 2);
    assert_eq!(coded_segment(0, 7, 7), 7);
    for coded in 0..8 {
        for pred in 0..8 {
            for last in 0..8u8 {
                assert!(coded_segment(coded, pred, last) <= usize::from(last));
            }
        }
    }
}

/** A tile group whose explicit range runs past the frame's tiles is refused. */
#[test]
fn a_tile_group_range_past_the_frame_is_refused() {
    with_frame(AIRBNB, |_, hdr, groups| {
        let mut bad = vec![0u8; groups[0].len()];
        bad[0] = 0;
        let mut single = hdr.clone();
        single.tiles.cols = 1;
        single.tiles.rows = 1;
        assert!(split_tiles(&single, &bad).is_ok());
        let mut two = hdr.clone();
        two.tiles.cols = 2;
        two.tiles.cols_log2 = 1;
        two.tiles.rows = 1;
        two.tiles.tile_size_bytes = 1;
        let bytes = [0b1100_0000u8, 0, 0];
        assert!(split_tiles(&two, &bytes).is_err());
    })
    .unwrap();
}

/** A tile group takes its last tile from the rest of the payload and earlier tiles from their explicit sizes. */
#[test]
fn a_tile_group_splits_into_sized_tiles() {
    with_frame(AIRBNB, |_, hdr, _| {
        let mut two = hdr.clone();
        two.tiles.cols = 2;
        two.tiles.cols_log2 = 1;
        two.tiles.rows = 1;
        two.tiles.tile_size_bytes = 1;
        let group = [0b0000_0000u8, 1, 0xAA, 0xBB, 0xCC];
        let tiles = split_tiles(&two, &group).unwrap();
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].data, &[0xAA, 0xBB]);
        assert_eq!(tiles[1].data, &[0xCC]);
    })
    .unwrap();
}

/** The Airbnb picture's luma, reconstructed before any loop filter, is within 25 dB of the JPEG of the same picture and has the same mean brightness. The JPEG is itself lossy, so this is a plausibility bound, not an exact match. */
#[test]
fn airbnb_luma_matches_the_jpeg_before_filters() {
    let (fs, width, height) = with_frame(AIRBNB, |seq, hdr, groups| {
        let fs = decode_frame(seq, hdr, groups).expect("the tiles decode");
        (
            fs,
            hdr.size.frame_width as usize,
            hdr.size.frame_height as usize,
        )
    })
    .expect("the headers parse");
    let (want, jw, jh) = jpeg_luma();
    assert_eq!((jw, jh), (width, height));
    let got: Vec<f64> = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .map(|(x, y)| f64::from(fs.planes[0].get(x, y)))
        .collect();
    let (mean_got, mean_want) = (mean(&got), mean(&want));
    assert!(psnr(&got, &want) > 25.0, "PSNR {:.2} dB", psnr(&got, &want));
    assert!(
        (mean_got - mean_want).abs() < 2.0,
        "means {mean_got:.1} vs {mean_want:.1}"
    );
}

/** The mean of a list of samples. */
fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

/** The reconstructed luma of each fixture that decodes end to end has plausible brightness and no saturated stripe inside the picture: no full row at 0 or at the 8-bit maximum, which is what a dropped or mis-signed residual produces. The padding past the picture is decoded but not shown, so it is not checked. */
#[test]
fn reconstructed_planes_have_no_saturation_stripes() {
    for (name, bytes) in SYNCED {
        let (fs, width, height) = with_frame(bytes, |seq, hdr, groups| {
            let fs = decode_frame(seq, hdr, groups).unwrap_or_else(|e| panic!("{name}: {e}"));
            (
                fs,
                hdr.size.frame_width as usize,
                hdr.size.frame_height as usize,
            )
        })
        .expect("the headers parse");
        let plane = &fs.planes[0];
        let picture = |y: usize| (0..width).map(move |x| plane.get(x, y));
        let total: f64 = (0..height).flat_map(picture).map(f64::from).sum();
        let mean_luma = total / (width * height) as f64;
        assert!(
            (20.0..235.0).contains(&mean_luma),
            "{name}: mean luma {mean_luma:.1}"
        );
        for y in 0..height {
            let stripe = picture(y).all(|v| v == 0) || picture(y).all(|v| v == 255);
            assert!(!stripe, "{name}: saturated row {y}");
        }
    }
}

/** A block in the middle of a 64 by 64 tile, with no chroma, for the intra block copy validity rules. */
fn dv_block(mv: [i32; 2]) -> intrabc_valid::DvBlock {
    intrabc_valid::DvBlock {
        mi_row: 16,
        mi_col: 16,
        width: 8,
        height: 8,
        chroma_sub: None,
        tile: TileBounds {
            mi_row_start: 0,
            mi_row_end: 64,
            mi_col_start: 0,
            mi_col_end: 64,
        },
        sb128: false,
        mv,
    }
}

/** A vector must name whole samples: a fractional component is refused, the same vector whole is accepted. */
#[test]
fn an_intrabc_vector_must_be_whole_samples() {
    assert!(intrabc_valid::dv_is_valid(&dv_block([
        -(16 * 8),
        -(16 * 8)
    ])));
    assert!(!intrabc_valid::dv_is_valid(&dv_block([
        -(16 * 8) + 1,
        -(16 * 8)
    ])));
    assert!(!intrabc_valid::dv_is_valid(&dv_block([
        -(16 * 8),
        -(16 * 8) - 3
    ])));
}

/** A source that starts outside the tile is refused, as is a vector past the magnitude limit. */
#[test]
fn an_intrabc_source_outside_the_tile_is_refused() {
    let mut block = dv_block([0, -(17 * 4 * 8)]);
    assert!(!intrabc_valid::dv_is_valid(&block), "source left of tile");
    block.mv = [-(17 * 4 * 8), 0];
    assert!(!intrabc_valid::dv_is_valid(&block), "source above tile");
    block.mv = [0, 1 << 14];
    assert!(!intrabc_valid::dv_is_valid(&block), "magnitude limit");
}

/** A source one superblock row up is inside the delay the decoder keeps, so it is refused. */
#[test]
fn an_intrabc_source_inside_the_delay_is_refused() {
    assert!(!intrabc_valid::dv_is_valid(&dv_block([-(16 * 8), 0])));
}

/** A source four superblocks back in raster order, inside the tile, satisfies the delay and is accepted. */
#[test]
fn an_intrabc_source_past_the_delay_is_accepted() {
    let mut block = dv_block([-(96 * 8), 0]);
    block.mi_row = 32;
    block.mi_col = 32;
    assert!(intrabc_valid::dv_is_valid(&block));
}

/** Whole-sample rounding of a candidate: the magnitude is rounded past half a sample and the sign kept. */
#[test]
fn a_candidate_rounds_to_the_nearest_whole_sample() {
    assert_eq!(mv_stack::to_whole_sample(12), 8);
    assert_eq!(mv_stack::to_whole_sample(-12), -8);
    assert_eq!(mv_stack::to_whole_sample(13), 16);
    assert_eq!(mv_stack::to_whole_sample(0), 0);
}

/** Candidates with the same vector merge their weights; a new vector is appended; a full stack ignores a new vector. */
#[test]
fn the_mv_stack_merges_weights_and_stops_at_capacity() {
    let mut stack = mv_stack::MvStack::default();
    stack.add([8, 8], 2);
    stack.add([8, 8], 3);
    assert_eq!(stack.len(), 1);
    for i in 0..MAX_REF_MV_STACK_SIZE as i32 {
        stack.add([16 * (i + 1), 0], 1);
    }
    assert_eq!(stack.len(), MAX_REF_MV_STACK_SIZE);
    assert_eq!(stack.mv_or_zero(MAX_REF_MV_STACK_SIZE), [0, 0]);
}

/** The sort orders each range by descending weight and keeps equal weights in their order. */
#[test]
fn the_mv_stack_sort_is_by_weight_and_stable() {
    let mut stack = mv_stack::MvStack::default();
    stack.add([8, 0], 1);
    stack.add([16, 0], 4);
    stack.add([24, 0], 4);
    stack.sort(0, stack.len());
    assert_eq!(stack.mv_or_zero(0), [16, 0]);
    assert_eq!(stack.mv_or_zero(1), [24, 0]);
    assert_eq!(stack.mv_or_zero(2), [8, 0]);
}

/** A region splits along its longer side, and into quarters when square; a width and height no transform has is malformed. */
#[test]
fn a_transform_region_splits_along_its_longer_side() {
    assert_eq!(
        tx_tree::split_region(0, 0, 64, 16),
        vec![(0, 0, 32, 16), (32, 0, 32, 16)]
    );
    assert_eq!(tx_tree::split_region(0, 0, 8, 16).len(), 2);
    assert_eq!(tx_tree::split_region(0, 0, 16, 16).len(), 4);
    assert!(tx_tree::find_tx_size(8, 64).is_err());
    assert_eq!(tx_tree::find_tx_size(16, 64).unwrap(), TX_16X64);
}
