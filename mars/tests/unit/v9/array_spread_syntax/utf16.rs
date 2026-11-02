/** Borrowed code-point iteration preserves JavaScript units without changing indexed strings. */
use super::*;

/** Returned slices alias the original allocation and pair only adjacent valid surrogate units. */
#[test]
fn cursor_borrows_pairs_and_preserves_lone_surrogates() {
    let string = JsString::from_units(vec![
        0x41, 0xd83d, 0xde00, 0x42, 0xd800, 0xd801, 0xdc00, 0xdfff,
    ])
    .unwrap();
    let mut cursor = string.code_points();
    let first = cursor.next().unwrap();
    assert_eq!(first.as_ptr(), string.as_units().as_ptr());
    assert_eq!(first, &[0x41]);
    assert_eq!(
        cursor.collect::<Vec<_>>(),
        vec![
            &[0xd83d, 0xde00][..],
            &[0x42],
            &[0xd800],
            &[0xd801, 0xdc00],
            &[0xdfff]
        ]
    );
    let empty = JsString::from_units(Vec::new()).unwrap();
    let mut cursor = empty.code_points();
    assert_eq!(cursor.next(), None);
    assert_eq!(cursor.next(), None);
}

/** Astral characters form one output element while combining marks remain independent. */
#[test]
fn spread_uses_code_points_without_grapheme_normalization() {
    assert_eq!(result("", "[...'A😀B'].length"), "3");
    assert_eq!(result("", "[...'e\\u0301'].length"), "2");
    assert_eq!(
        round_trip_units("const value=[...'A😀B'][1];", "value"),
        vec![0xd83d, 0xde00]
    );
    assert_eq!(result("", "[...''].length"), "0");
}

/** Unmatched code units survive spread and serialization rather than becoming replacement text. */
#[test]
fn unmatched_surrogates_survive_spread_private_wire_round_trips() {
    for (index, expected) in [(0, vec![0xd800]), (1, vec![0x78]), (2, vec![0xdfff])] {
        let source = format!("const value=[...'\\ud800x\\udfff'][{index}];");
        assert_eq!(round_trip_units(&source, "value"), expected);
    }
    assert_eq!(result("", "[...'\\ud800x\\udfff'].length"), "3");
}

/** Indexed reads and split remain code-unit operations alongside the new code-point cursor. */
#[test]
fn code_point_spread_does_not_change_code_unit_string_operations() {
    assert_eq!(
        result(
            "",
            "'😀'.length+' '+[...'😀'].length+' '+'😀'.split('').length"
        ),
        "2 1 2"
    );
    assert_eq!(
        round_trip_units("const value='😀'[0];", "value"),
        vec![0xd83d]
    );
}

/** A lone surrogate crosses the UTF-8 DOM boundary as U+FFFD instead of failing the write. */
#[test]
fn unmatched_surrogate_becomes_a_replacement_character_at_the_dom_write() {
    assert_eq!(result("const a=[...'\\ud800'];", "a[0]"), "\u{fffd}");
}
