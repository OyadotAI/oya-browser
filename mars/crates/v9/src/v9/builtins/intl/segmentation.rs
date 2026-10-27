//! Text segmentation shared by `Intl.Segmenter` and the conformance-only `Intl.v8BreakIterator`,
//! so both report the same boundaries. Each splitter walks the text once and counts UTF-16 units as
//! it goes, the unit pages index strings in, so no segment costs a rescan of the text before it.

/** One segment: where it starts in UTF-16 units, its text, and whether it is word-like. */
pub(in crate::v9::builtins) struct Piece {
    /** Offset of the first unit in the whole input. */
    pub start: usize,
    /** The segment's characters. */
    pub text: String,
    /** True for a run of letters and digits, which a word break iterator reports as a word. */
    pub wordy: bool,
}

/** The segments of `input` at `granularity`: "word" and "sentence" split as named, and any other
name, including "grapheme", yields one segment per scalar value. */
pub(in crate::v9::builtins) fn split(input: &str, granularity: &str) -> Vec<Piece> {
    match granularity {
        "word" => words(input),
        "sentence" => sentences(input),
        _ => characters(input),
    }
}

/** Each scalar value is its own segment: V9's grapheme granularity, a known simplification. */
fn characters(input: &str) -> Vec<Piece> {
    let mut pieces = Vec::with_capacity(input.len());
    let mut units = 0;
    for c in input.chars() {
        pieces.push(Piece {
            start: units,
            text: c.to_string(),
            wordy: false,
        });
        units += c.len_utf16();
    }
    pieces
}

/** Letters, digits and the apostrophe that joins contractions. */
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\''
}

/** A run of word characters is one segment, but a run that starts with an apostrophe is not
word-like, and every other character stands alone. */
fn words(input: &str) -> Vec<Piece> {
    let mut pieces: Vec<Piece> = Vec::new();
    let mut units = 0;
    for c in input.chars() {
        match pieces.last_mut() {
            Some(last) if is_word_char(c) && last.wordy => last.text.push(c),
            _ => pieces.push(Piece {
                start: units,
                text: c.to_string(),
                wordy: c.is_alphanumeric(),
            }),
        }
        units += c.len_utf16();
    }
    pieces
}

/** The sentence terminators that end a segment after themselves. */
fn is_terminator(c: char) -> bool {
    matches!(c, '.' | '!' | '?')
}

/** A segment runs up to and including a terminator, or to the end of the input. */
fn sentences(input: &str) -> Vec<Piece> {
    let mut pieces: Vec<Piece> = Vec::new();
    let mut units = 0;
    let mut ended = true;
    for c in input.chars() {
        if ended {
            pieces.push(Piece {
                start: units,
                text: String::new(),
                wordy: false,
            });
        }
        if let Some(last) = pieces.last_mut() {
            last.text.push(c);
        }
        ended = is_terminator(c);
        units += c.len_utf16();
    }
    pieces
}
