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
    /**
     * A Fenwick tree over the positions: entry p (1-based, so position p-1) counts the tokens present
     * in its span, so the k-th token in order is found in O(log n). Entry 0 is unused.
     */
    counts: Vec<i64>,
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

    /** Whether the set holds no tokens. */
    pub fn is_empty(&self) -> bool {
        self.position.is_empty()
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
        self.grow();
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
        self.add(at as usize + 1, -1);
        true
    }

    /** Replaces a token where it sits, and drops any other place `new` held. */
    pub fn replace(&mut self, old: &str, new: &str) -> bool {
        let Some(at) = self.position.remove(old) else {
            return false;
        };
        self.token.remove(&at);
        self.add(at as usize + 1, -1);
        self.remove(new);
        self.position.insert(new.to_string(), at);
        self.token.insert(at, new.to_string());
        self.add(at as usize + 1, 1);
        true
    }

    /** The token at `index` in order, found through the Fenwick tree in O(log n). */
    pub fn nth(&self, index: usize) -> Option<&str> {
        let at = self.position_of_rank(index)?;
        self.token.get(&at).map(String::as_str)
    }

    /** Appends a position for the token being added, with its present count. Its tree entry is
    the count plus the sums of the span it covers that already exist, so later sums stay exact. */
    fn grow(&mut self) {
        if self.counts.is_empty() {
            self.counts.push(0);
        }
        let index = self.counts.len();
        let below = self.prefix(index - 1) - self.prefix(index - lowest_bit(index));
        self.counts.push(1 + below);
    }

    /** Adds `delta` to the count at a 1-based tree index and every entry that covers it. */
    fn add(&mut self, mut index: usize, delta: i64) {
        while index < self.counts.len() {
            self.counts[index] += delta;
            index += lowest_bit(index);
        }
    }

    /** The number of present tokens in positions 1..=index, in O(log n). */
    fn prefix(&self, mut index: usize) -> i64 {
        let mut sum = 0;
        while index > 0 {
            sum += self.counts[index];
            index -= lowest_bit(index);
        }
        sum
    }

    /** The position of the token at rank `index` (0-based) among the present ones: a descent of the
    tree that finds the first position whose prefix count reaches index + 1, in O(log n). */
    fn position_of_rank(&self, index: usize) -> Option<u64> {
        let size = self.counts.len().checked_sub(1)?;
        if index >= self.len() {
            return None;
        }
        let mut rest = index as i64 + 1;
        let mut found = 0;
        let mut step = if size == 0 { 0 } else { 1 << (usize::BITS - 1 - size.leading_zeros()) };
        while step > 0 {
            if found + step <= size && self.counts[found + step] < rest {
                found += step;
                rest -= self.counts[found];
            }
            step >>= 1;
        }
        Some(found as u64)
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

/** The lowest set bit of a 1-based Fenwick index, which is the step to the next entry that covers it. */
fn lowest_bit(index: usize) -> usize {
    index & index.wrapping_neg()
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

    /** nth agrees with the order a plain list keeps through a mix of adds, removes and replaces. */
    #[test]
    fn nth_matches_a_plain_list_through_adds_removes_and_replaces() {
        let mut set = TokenSet::default();
        let mut model: Vec<String> = Vec::new();
        for step in 0..200u32 {
            let token = format!("t{}", (step * 7) % 23);
            if step % 3 == 0 {
                set.remove(&token);
                model.retain(|t| *t != token);
            } else if step % 5 == 0 {
                let other = format!("r{step}");
                if set.replace(&token, &other) {
                    let at = model.iter().position(|t| *t == token).expect("model holds it");
                    model[at] = other;
                    model.dedup();
                }
            } else if set.insert(&token) {
                model.push(token);
            }
            assert_eq!(set.len(), model.len());
            for (index, expected) in model.iter().enumerate() {
                assert_eq!(set.nth(index), Some(expected.as_str()), "step {step} index {index}");
            }
            assert_eq!(set.nth(model.len()), None);
        }
    }
}
