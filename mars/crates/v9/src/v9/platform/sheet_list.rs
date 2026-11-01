/**
 * A style sheet's rules as an editable list, so `insertRule` and `deleteRule` cost the rule they
 * touch rather than the whole sheet. A sheet's text is split into rules once, when a sheet method
 * first needs them. Edits change the list and mark the sheet dirty; the element's text is rebuilt
 * from the list only when something reads or changes the tree (`flush_sheet_edits`), so a page
 * that inserts thousands of rules in a row pays for one rebuild, not one per insert.
 */
use super::Page;
use super::sheet_rules::top_level_rules;
use std::collections::BTreeSet;

/** One top-level rule and the whitespace or comments before it. The pieces joined in order are
the sheet's text, byte for byte, until an edit changes them. */
#[derive(Clone, Debug)]
pub(in crate::v9) struct Piece {
    gap: String,
    rule: String,
}

/** A sheet's pieces, the text after its last rule, and the bookkeeping that says when the list
can be trusted: `dirty` marks edits the element's text does not carry yet, and `generation` is the
tree generation the list was read at. A list whose generation is no longer the page's was made
stale by a change to the tree, so it is read again from the text. */
#[derive(Clone, Debug, Default)]
pub(in crate::v9) struct SheetList {
    pieces: Vec<Piece>,
    tail: String,
    dirty: bool,
    generation: u64,
}

impl SheetList {
    /** Split `text` into its top-level rules, as a list read at `generation`. */
    pub(in crate::v9) fn parse(text: &str, generation: u64) -> Self {
        let mut pieces = Vec::new();
        let mut from = 0;
        for range in top_level_rules(text) {
            pieces.push(Piece {
                gap: text[from..range.start].to_string(),
                rule: text[range.clone()].to_string(),
            });
            from = range.end;
        }
        SheetList {
            pieces,
            tail: text[from..].to_string(),
            dirty: false,
            generation,
        }
    }

    /** The number of top-level rules. */
    pub(in crate::v9) fn len(&self) -> usize {
        self.pieces.len()
    }

    /** Each rule's text, trimmed, in order: what `cssText` reports. */
    pub(in crate::v9) fn rule_texts(&self) -> Vec<String> {
        self.pieces
            .iter()
            .map(|piece| piece.rule.trim().to_string())
            .collect()
    }

    /** Insert `rule` as the rule at `position`. The new rule gets its own line, and the rule that
    follows it gets one too, so the joined text never runs two rules together. */
    pub(in crate::v9) fn insert(&mut self, position: usize, rule: &str) {
        self.pieces.insert(
            position,
            Piece {
                gap: "\n".to_string(),
                rule: rule.to_string(),
            },
        );
        if let Some(next) = self.pieces.get_mut(position + 1) {
            next.gap.insert(0, '\n');
        }
    }

    /** Remove the rule at `position` with the whitespace before it. */
    pub(in crate::v9) fn remove(&mut self, position: usize) {
        self.pieces.remove(position);
    }

    /** The sheet's text: the pieces and the tail, joined. */
    pub(in crate::v9) fn text(&self) -> String {
        let mut out = String::new();
        for piece in &self.pieces {
            out.push_str(&piece.gap);
            out.push_str(&piece.rule);
        }
        out.push_str(&self.tail);
        out
    }

    /** Whether the list was read at `generation` and has not been edited since it was last
    flushed to the text: only then can it stand in for reading the text again. */
    fn is_current(&self, generation: u64) -> bool {
        self.generation == generation
    }
}

impl Page<'_> {
    /** Make a sheet's list current: read it from the element's text unless a list read at the
    tree's current generation is already held. */
    pub(in crate::v9) fn sheet_list(&mut self, node: usize) -> &SheetList {
        let generation = self.generation;
        if !self
            .state
            .sheet_lists
            .get(&node)
            .is_some_and(|list| list.is_current(generation))
        {
            let text = self.text_content(node);
            let list = SheetList::parse(&text, self.generation);
            self.state.sheet_lists.insert(node, list);
        }
        &self.state.sheet_lists[&node]
    }

    /** Apply an edit to a sheet's list, which must be current. The sheet is queued for a flush
    to its text unless it already is. */
    pub(in crate::v9) fn edit_sheet(&mut self, node: usize, edit: impl FnOnce(&mut SheetList)) {
        let Some(list) = self.state.sheet_lists.get_mut(&node) else {
            return;
        };
        edit(list);
        if !list.dirty {
            list.dirty = true;
            self.state.dirty_sheets.push(node);
        }
    }

    /** Write every edited sheet's text back to its element. Runs before anything reads or changes
    the tree, so no read sees a sheet's text behind its edits. A sheet whose text write fails stays
    queued and is tried again at the next flush. */
    pub(in crate::v9) fn flush_sheet_edits(&mut self) {
        if self.state.dirty_sheets.is_empty() {
            return;
        }
        let nodes = std::mem::take(&mut self.state.dirty_sheets);
        let failed: BTreeSet<usize> = nodes
            .iter()
            .copied()
            .filter(|node| !self.write_sheet_text(*node))
            .collect();
        self.settle_flushed(&nodes, &failed);
    }

    /** Write one sheet's list to its element. False when the write fails. */
    fn write_sheet_text(&mut self, node: usize) -> bool {
        let Some(text) = self.state.sheet_lists.get(&node).map(SheetList::text) else {
            return true;
        };
        self.set_text(node, &text).is_ok()
    }

    /** After a flush, mark the written sheets clean at the generation the writes left, and queue
    the failed ones again. The generation is read once, after every write, since each write moves
    it on. */
    fn settle_flushed(&mut self, nodes: &[usize], failed: &BTreeSet<usize>) {
        let generation = self.generation;
        for node in nodes {
            if let Some(list) = self.state.sheet_lists.get_mut(node) {
                list.dirty = failed.contains(node);
                list.generation = generation;
            }
        }
        self.state.dirty_sheets.extend(failed.iter().copied());
    }
}
