/** iloc extent bounds: crafted zero-width and oversized extent counts must fail fast and never reserve memory the input cannot back. */
// Each test binary uses a different subset of the writer.
#[allow(dead_code)]
mod support;

use mars_avif::{Error, dimensions};
use support::*;

/** The av01 item id the crafted files declare. */
const ITEM: u16 = 1;

/** A file whose single item has an iloc version 1 entry with the given field-width byte pair and extent count. */
fn crafted(widths: [u8; 2], count: u16) -> Vec<u8> {
    let mut body = vec![widths[0], widths[1]];
    body.extend_from_slice(&1u16.to_be_bytes());
    body.extend_from_slice(&ITEM.to_be_bytes());
    body.extend_from_slice(&0u16.to_be_bytes()); // construction method
    body.extend_from_slice(&0u16.to_be_bytes()); // data_reference_index
    body.extend_from_slice(&count.to_be_bytes());
    let props = [ispe(8, 8), av1c()];
    let meta_body = [
        hdlr(b"pict"),
        pitm(ITEM),
        iinf(&[infe(ITEM, b"av01")]),
        fb(b"iloc", 1, 0, &body),
        iprp(&props, &ipma(&[(ITEM, &[1, 2])])),
    ];
    [
        ftyp(b"avif", &[b"mif1"]),
        fb(b"meta", 0, 0, &meta_body.concat()),
    ]
    .concat()
}

#[test]
fn a_huge_extent_count_is_refused_before_any_reservation() {
    let file = crafted([0x00, 0x00], u16::MAX);
    assert_eq!(dimensions(&file), Err(Error::Limit("extent count")));
}

#[test]
fn many_zero_width_extents_in_one_location_are_refused() {
    let file = crafted([0x00, 0x00], 2);
    assert_eq!(
        dimensions(&file),
        Err(Error::Limit("zero-width extent count"))
    );
}

#[test]
fn one_zero_width_extent_is_allowed() {
    assert_eq!(dimensions(&crafted([0x00, 0x00], 1)), Ok((8, 8)));
}

#[test]
fn a_wide_extent_count_larger_than_the_remaining_bytes_is_truncated() {
    // Version 1 with 4-byte offset and length fields: 4000 extents need 32000 bytes, the file has far fewer.
    let file = crafted([0x44, 0x00], 4000);
    assert_eq!(dimensions(&file), Err(Error::Truncated));
}
