/** Character references decode with HTML's rules over a large owned named table, never growing text. */
/** Named references from HTML 4 and the common HTML5 additions; every name is at least two bytes,
so with `&` and `;` the reference is never shorter than the scalar it decodes to. */
const NAMED: &[(&str, char)] = &[
    ("AElig", 'Æ'),
    ("AMP", '&'),
    ("Aacute", 'Á'),
    ("Acirc", 'Â'),
    ("Agrave", 'À'),
    ("Alpha", 'Α'),
    ("Aring", 'Å'),
    ("Atilde", 'Ã'),
    ("Auml", 'Ä'),
    ("Beta", 'Β'),
    ("COPY", '©'),
    ("Ccedil", 'Ç'),
    ("Chi", 'Χ'),
    ("Dagger", '‡'),
    ("Delta", 'Δ'),
    ("ETH", 'Ð'),
    ("Eacute", 'É'),
    ("Ecirc", 'Ê'),
    ("Egrave", 'È'),
    ("Epsilon", 'Ε'),
    ("Eta", 'Η'),
    ("Euml", 'Ë'),
    ("GT", '>'),
    ("Gamma", 'Γ'),
    ("Iacute", 'Í'),
    ("Icirc", 'Î'),
    ("Igrave", 'Ì'),
    ("Iota", 'Ι'),
    ("Iuml", 'Ï'),
    ("Kappa", 'Κ'),
    ("LT", '<'),
    ("Lambda", 'Λ'),
    ("Mu", 'Μ'),
    ("NewLine", '\n'),
    ("Ntilde", 'Ñ'),
    ("Nu", 'Ν'),
    ("OElig", 'Œ'),
    ("Oacute", 'Ó'),
    ("Ocirc", 'Ô'),
    ("Ograve", 'Ò'),
    ("Omega", 'Ω'),
    ("Omicron", 'Ο'),
    ("Oslash", 'Ø'),
    ("Otilde", 'Õ'),
    ("Ouml", 'Ö'),
    ("Phi", 'Φ'),
    ("Pi", 'Π'),
    ("Prime", '″'),
    ("Psi", 'Ψ'),
    ("QUOT", '"'),
    ("REG", '®'),
    ("Rho", 'Ρ'),
    ("Scaron", 'Š'),
    ("Sigma", 'Σ'),
    ("THORN", 'Þ'),
    ("TRADE", '™'),
    ("Tab", '\t'),
    ("Tau", 'Τ'),
    ("Theta", 'Θ'),
    ("Uacute", 'Ú'),
    ("Ucirc", 'Û'),
    ("Ugrave", 'Ù'),
    ("Upsilon", 'Υ'),
    ("Uuml", 'Ü'),
    ("Xi", 'Ξ'),
    ("Yacute", 'Ý'),
    ("Yuml", 'Ÿ'),
    ("Zeta", 'Ζ'),
    ("aacute", 'á'),
    ("acirc", 'â'),
    ("acute", '´'),
    ("aelig", 'æ'),
    ("agrave", 'à'),
    ("alefsym", 'ℵ'),
    ("alpha", 'α'),
    ("amp", '&'),
    ("and", '∧'),
    ("ang", '∠'),
    ("apos", '\''),
    ("aring", 'å'),
    ("ast", '*'),
    ("asymp", '≈'),
    ("atilde", 'ã'),
    ("auml", 'ä'),
    ("bdquo", '„'),
    ("beta", 'β'),
    ("brvbar", '¦'),
    ("bsol", '\\'),
    ("bull", '•'),
    ("bullet", '•'),
    ("cap", '∩'),
    ("ccedil", 'ç'),
    ("cedil", '¸'),
    ("cent", '¢'),
    ("check", '✓'),
    ("chi", 'χ'),
    ("circ", 'ˆ'),
    ("clubs", '♣'),
    ("colon", ':'),
    ("comma", ','),
    ("commat", '@'),
    ("cong", '≅'),
    ("copy", '©'),
    ("crarr", '↵'),
    ("cross", '✗'),
    ("cup", '∪'),
    ("curren", '¤'),
    ("dArr", '⇓'),
    ("dagger", '†'),
    ("darr", '↓'),
    ("dash", '‐'),
    ("deg", '°'),
    ("delta", 'δ'),
    ("diams", '♦'),
    ("divide", '÷'),
    ("dollar", '$'),
    ("eacute", 'é'),
    ("ecirc", 'ê'),
    ("egrave", 'è'),
    ("empty", '∅'),
    ("emsp", '\u{2003}'),
    ("ensp", '\u{2002}'),
    ("epsilon", 'ε'),
    ("equals", '='),
    ("equiv", '≡'),
    ("eta", 'η'),
    ("eth", 'ð'),
    ("euml", 'ë'),
    ("euro", '€'),
    ("excl", '!'),
    ("exist", '∃'),
    ("fnof", 'ƒ'),
    ("forall", '∀'),
    ("frac12", '½'),
    ("frac13", '⅓'),
    ("frac14", '¼'),
    ("frac23", '⅔'),
    ("frac34", '¾'),
    ("frasl", '⁄'),
    ("gamma", 'γ'),
    ("ge", '≥'),
    ("grave", '`'),
    ("gt", '>'),
    ("hArr", '⇔'),
    ("harr", '↔'),
    ("hearts", '♥'),
    ("hellip", '…'),
    ("horbar", '―'),
    ("hyphen", '‐'),
    ("iacute", 'í'),
    ("icirc", 'î'),
    ("iexcl", '¡'),
    ("igrave", 'ì'),
    ("image", 'ℑ'),
    ("infin", '∞'),
    ("int", '∫'),
    ("iota", 'ι'),
    ("iquest", '¿'),
    ("isin", '∈'),
    ("iuml", 'ï'),
    ("kappa", 'κ'),
    ("lArr", '⇐'),
    ("lambda", 'λ'),
    ("lang", '⟨'),
    ("laquo", '«'),
    ("larr", '←'),
    ("lceil", '⌈'),
    ("lcub", '{'),
    ("ldquo", '“'),
    ("le", '≤'),
    ("lfloor", '⌊'),
    ("lowast", '∗'),
    ("lowbar", '_'),
    ("loz", '◊'),
    ("lpar", '('),
    ("lrm", '\u{200e}'),
    ("lsaquo", '‹'),
    ("lsqb", '['),
    ("lsquo", '‘'),
    ("lt", '<'),
    ("macr", '¯'),
    ("mdash", '—'),
    ("micro", 'µ'),
    ("middot", '·'),
    ("minus", '−'),
    ("mu", 'μ'),
    ("nabla", '∇'),
    ("nbsp", '\u{a0}'),
    ("ndash", '–'),
    ("ne", '≠'),
    ("ni", '∋'),
    ("not", '¬'),
    ("notin", '∉'),
    ("nsub", '⊄'),
    ("ntilde", 'ñ'),
    ("nu", 'ν'),
    ("num", '#'),
    ("oacute", 'ó'),
    ("ocirc", 'ô'),
    ("oelig", 'œ'),
    ("ograve", 'ò'),
    ("oline", '‾'),
    ("omega", 'ω'),
    ("omicron", 'ο'),
    ("oplus", '⊕'),
    ("or", '∨'),
    ("ordf", 'ª'),
    ("ordm", 'º'),
    ("oslash", 'ø'),
    ("otilde", 'õ'),
    ("otimes", '⊗'),
    ("ouml", 'ö'),
    ("para", '¶'),
    ("part", '∂'),
    ("percnt", '%'),
    ("period", '.'),
    ("permil", '‰'),
    ("perp", '⊥'),
    ("phi", 'φ'),
    ("pi", 'π'),
    ("piv", 'ϖ'),
    ("plus", '+'),
    ("plusmn", '±'),
    ("pound", '£'),
    ("prime", '′'),
    ("prod", '∏'),
    ("prop", '∝'),
    ("psi", 'ψ'),
    ("quest", '?'),
    ("quot", '"'),
    ("rArr", '⇒'),
    ("radic", '√'),
    ("rang", '⟩'),
    ("raquo", '»'),
    ("rarr", '→'),
    ("rceil", '⌉'),
    ("rcub", '}'),
    ("rdquo", '”'),
    ("real", 'ℜ'),
    ("reg", '®'),
    ("rfloor", '⌋'),
    ("rho", 'ρ'),
    ("rlm", '\u{200f}'),
    ("rpar", ')'),
    ("rsaquo", '›'),
    ("rsqb", ']'),
    ("rsquo", '’'),
    ("sbquo", '‚'),
    ("scaron", 'š'),
    ("sdot", '⋅'),
    ("sect", '§'),
    ("semi", ';'),
    ("shy", '\u{ad}'),
    ("sigma", 'σ'),
    ("sigmaf", 'ς'),
    ("sim", '∼'),
    ("sol", '/'),
    ("spades", '♠'),
    ("square", '□'),
    ("star", '☆'),
    ("starf", '★'),
    ("sub", '⊂'),
    ("sube", '⊆'),
    ("sum", '∑'),
    ("sup", '⊃'),
    ("sup1", '¹'),
    ("sup2", '²'),
    ("sup3", '³'),
    ("supe", '⊇'),
    ("szlig", 'ß'),
    ("tau", 'τ'),
    ("there4", '∴'),
    ("theta", 'θ'),
    ("thetasym", 'ϑ'),
    ("thinsp", '\u{2009}'),
    ("thorn", 'þ'),
    ("tilde", '˜'),
    ("times", '×'),
    ("trade", '™'),
    ("uArr", '⇑'),
    ("uacute", 'ú'),
    ("uarr", '↑'),
    ("ucirc", 'û'),
    ("ugrave", 'ù'),
    ("uml", '¨'),
    ("upsih", 'ϒ'),
    ("upsilon", 'υ'),
    ("uuml", 'ü'),
    ("verbar", '|'),
    ("vert", '|'),
    ("weierp", '℘'),
    ("xi", 'ξ'),
    ("yacute", 'ý'),
    ("yen", '¥'),
    ("yuml", 'ÿ'),
    ("zeta", 'ζ'),
    ("zwj", '\u{200d}'),
    ("zwnj", '\u{200c}'),
];

/** The legacy names HTML still decodes without a terminating semicolon. */
const LEGACY: &[&str] = &[
    "AElig", "AMP", "Aacute", "Acirc", "Agrave", "Aring", "Atilde", "Auml", "COPY", "Ccedil",
    "ETH", "Eacute", "Ecirc", "Egrave", "Euml", "GT", "Iacute", "Icirc", "Igrave", "Iuml", "LT",
    "Ntilde", "Oacute", "Ocirc", "Ograve", "Oslash", "Otilde", "Ouml", "QUOT", "REG", "THORN",
    "Uacute", "Ucirc", "Ugrave", "Uuml", "Yacute", "aacute", "acirc", "acute", "aelig", "agrave",
    "amp", "aring", "atilde", "auml", "brvbar", "ccedil", "cedil", "cent", "copy", "curren", "deg",
    "divide", "eacute", "ecirc", "egrave", "eth", "euml", "frac12", "frac14", "frac34", "gt",
    "iacute", "icirc", "iexcl", "igrave", "iquest", "iuml", "laquo", "lt", "macr", "micro",
    "middot", "nbsp", "not", "ntilde", "oacute", "ocirc", "ograve", "ordf", "ordm", "oslash",
    "otilde", "ouml", "para", "plusmn", "pound", "quot", "raquo", "reg", "sect", "shy", "sup1",
    "sup2", "sup3", "szlig", "thorn", "times", "uacute", "ucirc", "ugrave", "uml", "uuml",
    "yacute", "yen", "yuml",
];

/** The longest named reference in the table, so a lookup never scans past it. */
const LONGEST_NAME: usize = 8;

/** Windows-1252 meanings HTML gives numeric references in the C1 control range 0x80..=0x9F. */
const C1: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

/// Decodes character references in text or an attribute value. The output is never longer than
/// the input, so callers bound decoded text by the source bound. In attributes a legacy name
/// without its semicolon stays literal before `=` or an alphanumeric, as HTML requires.
pub(super) fn decode(source: &str, attribute: bool) -> String {
    let mut output = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find('&') {
        output.push_str(&rest[..start]);
        rest = &rest[start + 1..];
        match reference(rest, attribute) {
            Some((value, used)) => {
                output.push(value);
                rest = &rest[used..];
            }
            None => output.push('&'),
        }
    }
    output.push_str(rest);
    output
}

/** One reference after its `&`: the decoded scalar and the source bytes it consumed. */
fn reference(rest: &str, attribute: bool) -> Option<(char, usize)> {
    if let Some(number) = rest.strip_prefix('#') {
        return numeric(number).map(|(value, used)| (value, used + 1));
    }
    let name_end = rest
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(rest.len())
        .min(LONGEST_NAME + 1);
    let name = &rest[..name_end];
    if rest[name_end..].starts_with(';')
        && let Some(value) = lookup(name)
    {
        return Some((value, name_end + 1));
    }
    legacy(rest, name, attribute)
}

/** The longest legacy name prefixing the text, unless an attribute continues it. */
fn legacy(rest: &str, name: &str, attribute: bool) -> Option<(char, usize)> {
    let found = LEGACY
        .iter()
        .filter(|legacy| name.starts_with(**legacy))
        .max_by_key(|legacy| legacy.len())?;
    let next = rest[found.len()..].chars().next();
    if attribute && next.is_some_and(|c| c == '=' || c.is_ascii_alphanumeric()) {
        return None;
    }
    lookup(found).map(|value| (value, found.len()))
}

/** Exact table lookup; the table is sorted by byte order for binary search. */
fn lookup(name: &str) -> Option<char> {
    NAMED
        .binary_search_by(|(key, _)| key.cmp(&name))
        .ok()
        .map(|index| NAMED[index].1)
}

/** Decimal or hexadecimal references; invalid scalars become U+FFFD as in HTML. */
fn numeric(rest: &str) -> Option<(char, usize)> {
    let (digits, radix, prefix) = match rest.as_bytes().first() {
        Some(b'x' | b'X') => (&rest[1..], 16, 1),
        _ => (rest, 10, 0),
    };
    let end = digits
        .find(|c: char| !c.is_digit(radix))
        .unwrap_or(digits.len());
    if end == 0 {
        return None;
    }
    let value = u32::from_str_radix(&digits[..end], radix).unwrap_or(u32::MAX);
    let used = prefix + end + usize::from(digits[end..].starts_with(';'));
    let value = match value {
        0x80..=0x9f => C1[(value - 0x80) as usize],
        0 => char::REPLACEMENT_CHARACTER,
        _ => char::from_u32(value).unwrap_or(char::REPLACEMENT_CHARACTER),
    };
    Some((value, used))
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Binary search needs the table in byte order with no duplicates. */
    #[test]
    fn named_table_is_sorted() {
        assert!(NAMED.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(NAMED.iter().all(|(name, _)| name.len() <= LONGEST_NAME));
        assert!(LEGACY.iter().all(|name| lookup(name).is_some()));
    }

    /** Decoding never grows text, so source bounds hold for decoded text. */
    #[test]
    fn decoding_never_grows() {
        for (name, value) in NAMED {
            assert!(name.len() + 2 >= value.len_utf8(), "{name}");
        }
        assert_eq!(decode("&#0;&#x110000;&#128;", false), "\u{fffd}\u{fffd}€");
    }
}
