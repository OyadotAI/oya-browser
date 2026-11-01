/** Whole-box framing of an append: which top-level boxes a byte run completes, and which sizes are refused. */
mod support;

use mars_mp4::{BoxSpan, Error, complete_boxes};
use support::bx;

/** Two complete boxes come back with their kinds and byte ranges, header included. */
#[test]
fn complete_boxes_lists_each_whole_top_level_box() {
    let bytes = [bx(b"ftyp", &[1; 4]), bx(b"moof", &[2; 6])].concat();
    let spans = complete_boxes(&bytes).unwrap();
    assert_eq!(
        spans,
        vec![
            BoxSpan {
                kind: *b"ftyp",
                start: 0,
                end: 12
            },
            BoxSpan {
                kind: *b"moof",
                start: 12,
                end: 26
            },
        ]
    );
}

/** A box cut short at the end is left out, so the next append can complete it. */
#[test]
fn complete_boxes_leaves_a_cut_short_box_for_the_next_append() {
    let whole = bx(b"moof", &[7; 10]);
    let cut = [bx(b"ftyp", &[1; 4]), whole[..9].to_vec()].concat();
    let spans = complete_boxes(&cut).unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].end, 12);
}

/** A header that is not even complete yet is also left for later, not refused. */
#[test]
fn complete_boxes_leaves_a_partial_header_for_later() {
    assert!(complete_boxes(&[0, 0, 0]).unwrap().is_empty());
}

/** A size smaller than its own header can never be right, so the run is refused. */
#[test]
fn complete_boxes_refuses_a_size_smaller_than_its_header() {
    let bad = [0, 0, 0, 4, b'm', b'o', b'o', b'v'];
    assert_eq!(complete_boxes(&bad), Err(Error::Malformed("box size")));
}

/** A size of zero runs the box to the end of the input, so it is complete as given. */
#[test]
fn complete_boxes_takes_a_size_of_zero_to_the_end() {
    let bytes = [0, 0, 0, 0, b'm', b'd', b'a', b't', 9, 9, 9];
    let spans = complete_boxes(&bytes).unwrap();
    assert_eq!(
        spans,
        vec![BoxSpan {
            kind: *b"mdat",
            start: 0,
            end: 11
        }]
    );
}

/** A type that is not printable ASCII means the bytes are another format (an EBML header, say), so they are refused. */
#[test]
fn complete_boxes_refuses_a_type_that_is_not_ascii() {
    let ebml = [0x1a, 0x45, 0xdf, 0xa3, 0, 0, 0, 0x7f, 1, 2, 3, 4];
    assert_eq!(complete_boxes(&ebml), Err(Error::Malformed("box type")));
}
