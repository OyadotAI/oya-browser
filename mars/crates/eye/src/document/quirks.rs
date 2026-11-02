/** Quirks mode: whether a page's doctype puts Chromium in quirks mode, decided as HTML's
"initial" insertion mode does for the doctypes pages actually carry. */
/** Public identifier prefixes that select quirks mode whatever the system identifier. */
const QUIRKY_PUBLIC: [&str; 9] = [
    "-//w3c//dtd html 3",
    "-//w3c//dtd html 4.0 transitional",
    "-//w3c//dtd html 4.0 frameset",
    "-//ietf//dtd html",
    "-//w3o//dtd w3 html",
    "-//netscape comm. corp.//dtd html",
    "-//microsoft//dtd internet explorer",
    "-//softquad",
    "-//webtechs//dtd mozilla html",
];
/** Prefixes that select quirks mode only without a system identifier. */
const QUIRKY_WITHOUT_SYSTEM: [&str; 2] = [
    "-//w3c//dtd html 4.01 transitional",
    "-//w3c//dtd html 4.01 frameset",
];

/** Whether `source` lays out in quirks mode: no doctype before the first element, a doctype
 * that does not name `html`, or a legacy public identifier. */
pub(super) fn quirks(source: &str) -> bool {
    let Some(doctype) = doctype(source) else {
        return true;
    };
    let lower = doctype.to_ascii_lowercase();
    let mut words = lower.split_ascii_whitespace();
    if words.next() != Some("html") {
        return true;
    }
    let quoted: Vec<&str> = lower.split(['"', '\'']).skip(1).step_by(2).collect();
    let public = if lower.contains("public") {
        quoted.first().copied()
    } else {
        None
    };
    let system = quoted.get(usize::from(public.is_some())).is_some();
    public.is_some_and(|id| {
        QUIRKY_PUBLIC.iter().any(|prefix| id.starts_with(prefix))
            || (!system
                && QUIRKY_WITHOUT_SYSTEM
                    .iter()
                    .any(|prefix| id.starts_with(prefix)))
    })
}

/** The text inside the leading `<!doctype ...>`, past a byte order mark, whitespace and
 * comments; None when markup or text comes first. */
fn doctype(source: &str) -> Option<&str> {
    let mut rest = source.trim_start_matches('\u{feff}');
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.split_once("-->").map_or("", |(_, tail)| tail);
            continue;
        }
        let head = rest.get(..9)?;
        if !head.eq_ignore_ascii_case("<!doctype") {
            return None;
        }
        return rest[9..].split_once('>').map(|(inner, _)| inner.trim());
    }
}

#[cfg(test)]
mod tests {
    use super::quirks;

    /** Standards doctypes, legacy strict ones and the transitional one with a system id. */
    #[test]
    fn standards_doctypes_are_not_quirky() {
        for source in [
            "<!DOCTYPE html><p>",
            "\u{feff}<!-- c --> <!doctype HTML>",
            "<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\">",
            "<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01 Transitional//EN\" \"http://www.w3.org/TR/html4/loose.dtd\">",
        ] {
            assert!(!quirks(source), "{source}");
        }
    }

    /** A missing doctype and legacy identifiers select quirks mode. */
    #[test]
    fn missing_and_legacy_doctypes_are_quirky() {
        for source in [
            "<html><body>",
            "<HEADER><TITLE>The World Wide Web project</TITLE>",
            "<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01 Transitional//EN\">",
            "<!DOCTYPE HTML PUBLIC \"-//IETF//DTD HTML 2.0//EN\">",
        ] {
            assert!(quirks(source), "{source}");
        }
    }
}
