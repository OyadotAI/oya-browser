/** Dictionary line breaking for scripts written without spaces between words (Thai, Lao,
 * Khmer, Myanmar), which UAX #14 leaves to a dictionary (class SA): the OS's tokenizer finds the
 * word boundaries a line may break at, as Blink asks ICU's dictionary break iterator. */
/** Whether `c` belongs to a script that needs a dictionary to find its words. */
pub fn dictionary(c: char) -> bool {
    matches!(c as u32, 0x0e00..=0x0eff | 0x1000..=0x109f | 0x1780..=0x17ff | 0x19e0..=0x19ff)
}

/** Byte offsets strictly inside `text` where its words meet, by the OS's line-break tokenizer;
 * empty off macOS, where such text keeps breaking only at spaces. */
#[cfg(target_os = "macos")]
pub fn word_boundaries(text: &str) -> Vec<usize> {
    use objc2_core_foundation::{
        CFRange, CFString, CFStringTokenizer, CFStringTokenizerTokenType,
        kCFStringTokenizerUnitLineBreak,
    };
    let string = CFString::from_str(text);
    let units: Vec<usize> = utf16_to_bytes(text);
    let range = CFRange::new(0, (units.len() - 1) as isize);
    // SAFETY: a tokenizer over a valid string and its own full range, default allocator and
    // locale.
    let Some(tokenizer) = (unsafe {
        CFStringTokenizer::new(
            None,
            Some(&string),
            range,
            kCFStringTokenizerUnitLineBreak,
            None,
        )
    }) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    while tokenizer.advance_to_next_token() != CFStringTokenizerTokenType::None {
        let start = tokenizer.current_token_range().location;
        if let Some(&at) = usize::try_from(start).ok().and_then(|i| units.get(i))
            && at > 0
            && at < text.len()
        {
            out.push(at);
        }
    }
    out
}

/** Off macOS no tokenizer is available. */
#[cfg(not(target_os = "macos"))]
pub fn word_boundaries(_text: &str) -> Vec<usize> {
    Vec::new()
}

/** For each UTF-16 unit index of `text`, its byte offset, plus one past the end. */
#[cfg(target_os = "macos")]
fn utf16_to_bytes(text: &str) -> Vec<usize> {
    let mut out = Vec::with_capacity(text.len() + 1);
    for (at, c) in text.char_indices() {
        out.extend(std::iter::repeat_n(at, c.len_utf16()));
    }
    out.push(text.len());
    out
}
