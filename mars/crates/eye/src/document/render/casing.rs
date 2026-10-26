/**
 * Language-sensitive case mapping for `text-transform`, after Blink's CaseMap with ICU's
 * tailorings: uppercase Greek drops its accents (keeping a diaeresis, moving the accent of a
 * diphthong's first vowel to a diaeresis on the second, and keeping the accent of a lone
 * disjunctive eta), Dutch capitalizes "ij" as one letter, Turkish and Azeri map dotted and
 * dotless i, and capitalize uses the titlecase of the Latin digraphs (`ǆ` gives `ǅ`).
 */
/** Whether `lang` (lowercase) is `code` or one of its subtags, written in the language's usual
 * script: an explicit other script subtag (`tr-Cyrl`) turns the tailoring off. */
fn is(lang: &str, code: &str) -> bool {
    let mut tags = lang.split('-');
    if tags.next() != Some(code) {
        return false;
    }
    let usual = if code == "el" { "grek" } else { "latn" };
    tags.next()
        .filter(|tag| tag.len() == SCRIPT_TAG)
        .is_none_or(|script| script == usual)
}

/// The length of a BCP 47 script subtag (`Latn`, `Cyrl`).
const SCRIPT_TAG: usize = 4;

/** The Greek vowels with a tonos (lowercase and capital) and their bare capitals. */
const GREEK_TONOS: [(char, char); 14] = [
    ('ά', 'Α'),
    ('έ', 'Ε'),
    ('ή', 'Η'),
    ('ί', 'Ι'),
    ('ό', 'Ο'),
    ('ύ', 'Υ'),
    ('ώ', 'Ω'),
    ('Ά', 'Α'),
    ('Έ', 'Ε'),
    ('Ή', 'Η'),
    ('Ί', 'Ι'),
    ('Ό', 'Ο'),
    ('Ύ', 'Υ'),
    ('Ώ', 'Ω'),
];

/** The bare capital of a Greek vowel with a tonos, if `c` is one. */
fn untoned(c: char) -> Option<char> {
    GREEK_TONOS
        .iter()
        .find(|(toned, _)| *toned == c)
        .map(|(_, bare)| *bare)
}

/** Uppercase Greek: accents dropped, a diaeresis kept (`ΐ` gives `Ϊ`), a diphthong's accent on
 * its first vowel moved to a diaeresis on its second (`άι` gives `ΑΪ`), and a lone `ή` keeps
 * its accent (`Ή`). */
fn greek_upper(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut diaeresis = false;
    for (index, &c) in chars.iter().enumerate() {
        let before = index.checked_sub(1).map(|i| chars[i]);
        let after = chars.get(index + 1).copied();
        let alone =
            !before.is_some_and(char::is_alphabetic) && !after.is_some_and(char::is_alphabetic);
        match c {
            'ή' if alone => out.push('Ή'),
            'ΐ' => out.push('Ϊ'),
            'ΰ' => out.push('Ϋ'),
            'ι' | 'υ' if diaeresis => out.push(if c == 'ι' { 'Ϊ' } else { 'Ϋ' }),
            _ => match untoned(c) {
                Some(bare) => out.push(bare),
                None => out.extend(c.to_uppercase()),
            },
        }
        diaeresis = matches!(c, 'ά' | 'έ' | 'ό' | 'ύ' | 'Ά' | 'Έ' | 'Ό' | 'Ύ');
    }
    out
}

/** `source` uppercased for `lang`. */
pub(super) fn upper(source: &str, lang: &str) -> String {
    if is(lang, "el") {
        return greek_upper(source);
    }
    if is(lang, "tr") || is(lang, "az") {
        return source.replace('i', "\u{130}").to_uppercase();
    }
    source.to_uppercase()
}

/** `source` lowercased for `lang`. */
pub(super) fn lower(source: &str, lang: &str) -> String {
    if is(lang, "tr") || is(lang, "az") {
        return source
            .replace('I', "\u{131}")
            .replace('\u{130}', "i")
            .to_lowercase();
    }
    source.to_lowercase()
}

/** The titlecase of `c`: the digraphs' own titlecase forms, else its uppercase. */
fn title(c: char, out: &mut String) {
    match c {
        '\u{1c4}'..='\u{1c6}' => out.push('\u{1c5}'),
        '\u{1c7}'..='\u{1c9}' => out.push('\u{1c8}'),
        '\u{1ca}'..='\u{1cc}' => out.push('\u{1cb}'),
        '\u{1f1}'..='\u{1f3}' => out.push('\u{1f2}'),
        _ => out.extend(c.to_uppercase()),
    }
}

/** `source` with each word's first letter in titlecase for `lang` (Dutch "ij" as one letter). */
pub(super) fn capitalize(source: &str, lang: &str) -> String {
    let dutch = is(lang, "nl");
    let mut out = String::with_capacity(source.len());
    let mut start = true;
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        if start && c.is_alphanumeric() {
            title(c, &mut out);
            if dutch && c == 'i' && chars.peek() == Some(&'j') {
                chars.next();
                out.push('J');
            }
        } else {
            out.push(c);
        }
        start = c.is_whitespace();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{capitalize, upper};

    /** Greek uppercase drops accents, keeps diaereses and a lone eta's accent. */
    #[test]
    fn greek_uppercase_tailoring() {
        assert_eq!(upper("καλημέρα αύριο", "el"), "ΚΑΛΗΜΕΡΑ ΑΥΡΙΟ");
        assert_eq!(upper("ευφυΐα Νεράιδα", "el"), "ΕΥΦΥΪΑ ΝΕΡΑΪΔΑ");
        assert_eq!(upper("ήσουν ή εγώ", "el"), "ΗΣΟΥΝ Ή ΕΓΩ");
    }

    /** Dutch capitalizes "ij" together; digraphs take their titlecase. */
    #[test]
    fn dutch_and_digraph_titlecase() {
        assert_eq!(capitalize("ijsland", "nl"), "IJsland");
        assert_eq!(capitalize("\u{1c6}a", "en"), "\u{1c5}a");
    }
}
