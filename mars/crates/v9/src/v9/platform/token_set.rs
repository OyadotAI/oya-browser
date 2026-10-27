/**
 * The ordered token set a DOMTokenList reads and writes, kept beside its attribute. Membership
 * and each add or remove cost O(log n) in the token count, where re-splitting the attribute text
 * on every call cost O(tokens) per call. The attribute stays a string: `serialize` rebuilds it
 * when the set changed, which is the one O(tokens) cost left, and only a write pays it.
 */
use std::collections::BTreeMap;

/** A set of tokens in insertion order: membership by token, order by position. */
#[derive(Clone, Debug, Default)]
pub(in crate::v9) struct TokenSet {
    /** The attribute text this set was read from, or last wrote; a set is reused only while it matches. */
    source: String,
    /** Each token's position in the order, for membership and removal. */
    position: BTreeMap<String, u64>,
    /** Tokens by position, so iteration follows the attribute's order. */
    token: BTreeMap<u64, String>,
    /** The position the next added token takes; positions only grow, so order is kept on removal. */
    next: u64,
}

impl TokenSet {
    /** Reads a set from attribute text: ASCII whitespace separates tokens, and a repeat keeps its first place. */
    pub fn parse(source: &str) -> Self {
        let mut set = Self {
            source: source.to_string(),
            ..Self::default()
        };
        for token in source.split_ascii_whitespace() {
            set.insert(token);
        }
        set
    }

    /** The attribute text the set was read from or last written as. */
    pub fn source(&self) -> &str {
        &self.source
    }

    /** Records that the attribute now holds `text`, so the next read of that text reuses this set. */
    pub fn mark_written(&mut self, text: String) {
        self.source = text;
    }

    /** How many tokens the set holds. */
    pub fn len(&self) -> usize {
        self.position.len()
    }

    /** Whether a token is in the set, in O(log n). */
    pub fn contains(&self, token: &str) -> bool {
        self.position.contains_key(token)
    }

    /** Adds a token at the end unless it is already there; returns whether it was added. */
    pub fn insert(&mut self, token: &str) -> bool {
        if self.contains(token) {
            return false;
        }
        let at = self.next;
        self.next += 1;
        self.position.insert(token.to_string(), at);
        self.token.insert(at, token.to_string());
        true
    }

    /** Removes a token, keeping the order of the rest; returns whether it was there. */
    pub fn remove(&mut self, token: &str) -> bool {
        let Some(at) = self.position.remove(token) else {
            return false;
        };
        self.token.remove(&at);
        true
    }

    /** Replaces a token where it sits, and drops any other place `new` held. */
    pub fn replace(&mut self, old: &str, new: &str) -> bool {
        let Some(at) = self.position.remove(old) else {
            return false;
        };
        self.token.remove(&at);
        self.remove(new);
        self.position.insert(new.to_string(), at);
        self.token.insert(at, new.to_string());
        true
    }

    /** The token at `index` in order. Reaching it walks past the earlier tokens, O(index). */
    pub fn nth(&self, index: usize) -> Option<&str> {
        self.token.values().nth(index).map(String::as_str)
    }

    /** The tokens in order. */
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.token.values().map(String::as_str)
    }

    /** The attribute text: the tokens in order, joined by one space. O(tokens), paid only on a write. */
    pub fn serialize(&self) -> String {
        self.iter().collect::<Vec<_>>().join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::TokenSet;

    /** Parsing keeps the first place of a repeated token and splits on ASCII whitespace only. */
    #[test]
    fn parse_keeps_first_places_and_splits_on_ascii_space() {
        let set = TokenSet::parse("b a\tb\na");
        assert_eq!(set.iter().collect::<Vec<_>>(), ["b", "a"]);
        assert_eq!(set.serialize(), "b a");
    }

    /** Removing a token keeps the order of the others, and a later add goes at the end. */
    #[test]
    fn remove_keeps_order_and_add_goes_last() {
        let mut set = TokenSet::parse("a b c");
        assert!(set.remove("b"));
        assert!(!set.remove("b"));
        assert!(set.insert("d"));
        assert_eq!(set.serialize(), "a c d");
    }

    /** Replace keeps the old token's place and leaves a single copy of the new one. */
    #[test]
    fn replace_keeps_the_place_and_drops_other_copies() {
        let mut set = TokenSet::parse("a b c");
        assert!(set.replace("a", "c"));
        assert_eq!(set.serialize(), "c b");
        assert!(!set.replace("zz", "q"));
    }

    /** nth reads by position, and past the end it is None. */
    #[test]
    fn nth_reads_by_position() {
        let set = TokenSet::parse("x y");
        assert_eq!(set.nth(1), Some("y"));
        assert_eq!(set.nth(2), None);
    }

    /** A set remembers the text it was read from until a write marks new text. */
    #[test]
    fn source_tracks_the_text_last_read_or_written() {
        let mut set = TokenSet::parse("a");
        assert_eq!(set.source(), "a");
        set.mark_written("a b".into());
        assert_eq!(set.source(), "a b");
    }
}
