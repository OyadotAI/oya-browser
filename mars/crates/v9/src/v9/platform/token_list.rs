/**
 * The rules a DOMTokenList answers to, as pure functions of strings: a token is non-empty and holds
 * no ASCII whitespace (SyntaxError and InvalidCharacterError), the methods that change a set must
 * check their tokens before any change, and `supports` reads the token list an attribute accepts.
 * Keeping these free of the VM lets each rule be tested on its own.
 */
use super::namespaces::{INVALID_CHARACTER_ERROR, NameError};

/** The DOM error an empty token raises. */
pub(in crate::v9) const SYNTAX_ERROR: &str = "SyntaxError";
/** The attribute whose token list `relList` reads and writes. */
const REL_ATTRIBUTE: &str = "rel";
/** The tag of a link element, whose `rel` has its own supported tokens. */
const LINK_TAG: &str = "link";
/** The tags whose `rel` supports the noopener and noreferrer keywords. */
const ANCHOR_TAGS: [&str; 2] = ["a", "area"];
/** The tokens a link element's `rel` supports, as the HTML standard lists them. */
const LINK_REL_TOKENS: &[&str] = &[
    "alternate",
    "dns-prefetch",
    "icon",
    "modulepreload",
    "pingback",
    "preconnect",
    "prefetch",
    "preload",
    "stylesheet",
];
/** The tokens an anchor's or area's `rel` supports. */
const ANCHOR_REL_TOKENS: &[&str] = &["noopener", "noreferrer"];
/** Methods that change the token set, and so run the update steps and check their tokens. */
pub(in crate::v9) const MUTATING_METHODS: [&str; 4] = ["add", "remove", "toggle", "replace"];
/** The arguments `replace` needs: the token and its replacement. */
const REPLACE_ARGUMENTS: usize = 2;

/** Checks one token: empty is a SyntaxError, and any ASCII whitespace an InvalidCharacterError. */
pub(in crate::v9) fn check_token(token: &str) -> Result<(), NameError> {
    if token.is_empty() {
        return Err(NameError {
            name: SYNTAX_ERROR,
            message: "The token provided must not be empty.".to_string(),
        });
    }
    if token
        .chars()
        .any(|character| character.is_ascii_whitespace())
    {
        return Err(NameError {
            name: INVALID_CHARACTER_ERROR,
            message: format!("The token provided ('{token}') contains HTML space characters."),
        });
    }
    Ok(())
}

/** Checks every token given, stopping at the first bad one; callers check before they change. */
pub(in crate::v9) fn check_tokens(tokens: &[String]) -> Result<(), NameError> {
    tokens.iter().try_for_each(|token| check_token(token))
}

/** How many leading arguments a DOMTokenList method requires, as WebIDL counts them. */
pub(in crate::v9) fn required_arguments(method: &str) -> usize {
    match method {
        "replace" => REPLACE_ARGUMENTS,
        "toggle" | "contains" | "item" | "supports" => 1,
        _ => 0,
    }
}

/** The tokens `supports` accepts for an element's attribute, or None when that list has none,
which is the case for classList, so `supports` on it throws. */
pub(in crate::v9) fn supported_tokens(
    tag: &str,
    attribute: &str,
) -> Option<&'static [&'static str]> {
    if attribute != REL_ATTRIBUTE {
        return None;
    }
    if tag == LINK_TAG {
        return Some(LINK_REL_TOKENS);
    }
    ANCHOR_TAGS.contains(&tag).then_some(ANCHOR_REL_TOKENS)
}

/** Whether a supported list names a token; the comparison ignores ASCII case, as `supports` does. */
pub(in crate::v9) fn is_supported(list: &[&str], token: &str) -> bool {
    list.iter().any(|known| known.eq_ignore_ascii_case(token))
}

#[cfg(test)]
mod tests {
    use super::{check_token, is_supported, supported_tokens};

    /** An empty token is a SyntaxError, the one rule that uses that name. */
    #[test]
    fn empty_token_is_a_syntax_error() {
        let error = check_token("").expect_err("empty token is rejected");
        assert_eq!(error.name, "SyntaxError");
    }

    /** A token with any ASCII whitespace is an InvalidCharacterError, while a plain one passes. */
    #[test]
    fn whitespace_in_a_token_is_an_invalid_character_error() {
        for bad in ["a b", "a\tb", "a\nb", "a\u{c}b", "a\rb"] {
            let error = check_token(bad).expect_err("whitespace is rejected");
            assert_eq!(error.name, "InvalidCharacterError", "{bad:?}");
        }
        assert!(check_token("plain-token").is_ok());
    }

    /** classList has no supported tokens; relList on a link or an anchor has a fixed list. */
    #[test]
    fn supported_lists_follow_the_attribute_and_tag() {
        assert!(supported_tokens("div", "class").is_none());
        assert!(supported_tokens("div", "rel").is_none());
        let link = supported_tokens("link", "rel").expect("link has a list");
        assert!(is_supported(link, "STYLESHEET"));
        assert!(!is_supported(link, "noopener"));
        let anchor = supported_tokens("a", "rel").expect("anchor has a list");
        assert!(is_supported(anchor, "noopener"));
    }
}
