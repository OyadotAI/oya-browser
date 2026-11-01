/** Symbol decoder tests: tiny bitstreams whose decoded symbols follow from the spec arithmetic by hand, CDF adaptation, and a real tile from the Airbnb fixture. */
use super::SymbolDecoder;
use crate::cdf_tables::DEFAULT_INTRA_FRAME_Y_MODE_CDF;
use crate::container;
use crate::error::Error;
use crate::frame::Parsed;
use crate::stream;

/** The Airbnb CDN AVIF, whose first tile supplies real symbol data. */
const AIRBNB: &[u8] = include_bytes!("../tests/fixtures/airbnb-240.avif");

/** Two bytes of zeros. Init gives SymbolValue 32767 and SymbolRange 32768; the first read_bool picks symbol 0. */
const ZEROS: [u8; 2] = [0x00, 0x00];
/** Two bytes of ones. Init gives SymbolValue 0, so the first read_bool picks symbol 1. */
const ONES: [u8; 2] = [0xff, 0xff];

#[test]
fn an_all_zero_stream_decodes_an_equiprobable_zero() {
    let mut d = SymbolDecoder::new(&ZEROS, false).unwrap();
    assert!(!d.read_bool().unwrap());
}

#[test]
fn an_all_one_stream_decodes_an_equiprobable_one() {
    let mut d = SymbolDecoder::new(&ONES, false).unwrap();
    assert!(d.read_bool().unwrap());
}

#[test]
fn read_literal_is_the_bools_most_significant_first() {
    let mut a = SymbolDecoder::new(&ONES, false).unwrap();
    let mut b = SymbolDecoder::new(&ONES, false).unwrap();
    let bits: Vec<u32> = (0..4).map(|_| u32::from(a.read_bool().unwrap())).collect();
    let packed = bits.iter().fold(0, |acc, bit| (acc << 1) | bit);
    assert_eq!(b.read_literal(4).unwrap(), packed);
}

#[test]
fn a_zero_decoded_symbol_moves_its_cdf_entry_up_by_one_sixteenth() {
    // N = 2, cdf = [16384, 32768, 0]. Symbol 0 is decoded; rate = 3 + 0 + 0 + min(floor(log2 2), 2) = 4.
    // cdf[0] += (32768 - 16384) >> 4 = 1024, and the count becomes 1.
    let mut d = SymbolDecoder::new(&ZEROS, false).unwrap();
    let mut cdf = [16384u16, 32768, 0];
    assert_eq!(d.read_symbol(&mut cdf).unwrap(), 0);
    assert_eq!(cdf, [17408, 32768, 1]);
}

#[test]
fn a_one_decoded_symbol_moves_the_first_entry_down_by_one_sixteenth() {
    // Same CDF and rate, symbol 1 decoded: cdf[0] -= (16384 - 0) >> 4 = 1024.
    let mut d = SymbolDecoder::new(&ONES, false).unwrap();
    let mut cdf = [16384u16, 32768, 0];
    assert_eq!(d.read_symbol(&mut cdf).unwrap(), 1);
    assert_eq!(cdf, [15360, 32768, 1]);
}

#[test]
fn the_adaptation_counter_saturates_at_thirty_two() {
    let mut d = SymbolDecoder::new(&ZEROS, false).unwrap();
    let mut cdf = [16384u16, 32768, 32];
    d.read_symbol(&mut cdf).unwrap();
    assert_eq!(cdf[2], 32);
}

#[test]
fn disabling_cdf_updates_leaves_the_cdf_alone() {
    let mut d = SymbolDecoder::new(&ZEROS, true).unwrap();
    let mut cdf = [16384u16, 32768, 0];
    assert_eq!(d.read_symbol(&mut cdf).unwrap(), 0);
    assert_eq!(cdf, [16384, 32768, 0]);
}

/** exit_symbol needs the trailing one bit the encoder writes; a tile of zero bytes has none, so the check refuses it. The passing case is covered by the fixture tiles, which end on their real trailing bits. */
#[test]
fn exit_refuses_a_tile_without_its_trailing_bit() {
    let mut d = SymbolDecoder::new(&ZEROS, false).unwrap();
    d.read_bool().unwrap();
    assert_eq!(d.exit(), Err(Error::Malformed("tile trailing bit missing")));
}

#[test]
fn an_empty_tile_is_malformed() {
    assert!(matches!(
        SymbolDecoder::new(&[], false),
        Err(Error::Malformed(_))
    ));
}

#[test]
fn a_cdf_without_a_terminal_entry_is_refused_not_scanned_past() {
    // cdf = [0, 0, 0] gives every symbol the full range, so the scan reaches symbol 2 (= N) and must stop there.
    let mut d = SymbolDecoder::new(&ONES, false).unwrap();
    let mut cdf = [0u16, 0, 0];
    assert_eq!(
        d.read_symbol(&mut cdf),
        Err(Error::Malformed("CDF has no terminal entry"))
    );
}

#[test]
fn the_first_tile_of_the_airbnb_frame_initialises_and_decodes_symbols() {
    let picture = container::open(AIRBNB).unwrap();
    let payload = picture.payload(picture.primary).unwrap();
    let headers = stream::parse_headers(&payload, &picture.av1c).unwrap();
    let Parsed::Frame(frame) = headers.frame else {
        panic!("expected a frame header");
    };
    let tile = &payload[frame.header_bytes..];
    let mut d = SymbolDecoder::new(tile, frame.disable_cdf_update).unwrap();
    let mut cdf = DEFAULT_INTRA_FRAME_Y_MODE_CDF[0][0];
    for _ in 0..32 {
        let mode = d.read_symbol(&mut cdf).unwrap();
        assert!(mode < 13, "intra y mode {mode} is outside the 13 modes");
    }
}
