/**
 * `Intl.Segmenter` after its splitter moved into the shared segmentation module that the
 * conformance break iterator also uses. These pin the segment text, the UTF-16 index, and the
 * word-like flag, so the shared splitter cannot drift from what pages already receive.
 */
use crate::common;

use common::eval;

/** Word segments carry their UTF-16 start and whether they are word-like. */
#[test]
fn word_segments_report_index_and_word_likeness() {
    let out = eval(
        "const out = [...new Intl.Segmenter('en', { granularity: 'word' }).segment('Hi, there!')]
           .map((s) => s.segment + ':' + s.index + ':' + s.isWordLike).join('|');",
        "out",
    );
    assert_eq!(out, "Hi:0:true|,:2:false| :3:false|there:4:true|!:9:false");
}

/** An apostrophe inside a word stays in the word, so a contraction is one segment. */
#[test]
fn contractions_stay_one_word_segment() {
    let out = eval(
        "const out = [...new Intl.Segmenter('en', { granularity: 'word' }).segment(\"don't\")]
           .map((s) => s.segment).join('|');",
        "out",
    );
    assert_eq!(out, "don't");
}

/** Grapheme segments are one per scalar, and their index counts UTF-16 units before them. */
#[test]
fn grapheme_index_counts_utf16_units() {
    let out = eval(
        "const out = [...new Intl.Segmenter('en').segment('\u{1F600}a')]
           .map((s) => s.index).join();",
        "out",
    );
    assert_eq!(out, "0,2");
}

/** Sentence segments end after a terminator and carry the rest of the text as the last one. */
#[test]
fn sentence_segments_end_at_terminators() {
    let out = eval(
        "const out = [...new Intl.Segmenter('en', { granularity: 'sentence' }).segment('One. Two! Three')]
           .map((s) => s.segment).join('|');",
        "out",
    );
    assert_eq!(out, "One.| Two!| Three");
}
