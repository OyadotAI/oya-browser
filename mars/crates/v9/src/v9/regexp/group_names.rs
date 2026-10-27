/**
 * Named capture groups while a pattern is parsed. ES2025 lets one name label several groups when
 * they sit in different alternatives of a disjunction; two carriers that can both participate in
 * one match (same sequence, or one nested in the other) are an early SyntaxError. Scopes are kept
 * as frames so each declaration checks only the names still open around it.
 */
use super::PatternError;
use std::collections::BTreeMap;

/** Group names in source order, the scopes that declare them, and a name-to-slot index. */
#[derive(Debug, Default)]
pub(super) struct GroupNames {
    /** Each name with every capture index that carries it, in order of first declaration. */
    pub(super) entries: Vec<(String, Vec<usize>)>,
    /** Position in `entries` for each name, so the lookup is logarithmic. */
    pub(super) slots: BTreeMap<String, usize>,
    /** Names declared directly in each open sequence; closed alternatives hand theirs upward. */
    frames: Vec<Vec<String>>,
    /** How many open frames hold each name; any count above zero is a conflict. */
    open: BTreeMap<String, usize>,
}

impl GroupNames {
    /** Opens the scope of one alternative (or of the whole pattern). */
    pub(super) fn open_frame(&mut self) {
        self.frames.push(Vec::new());
    }
    /** Closes the innermost scope and returns the names it declared, releasing them. */
    pub(super) fn close_frame(&mut self) -> Vec<String> {
        let names = self.frames.pop().unwrap_or_default();
        for name in &names {
            self.release(name);
        }
        names
    }
    /** Drops one open claim on a name, forgetting it once no open scope holds it. */
    fn release(&mut self, name: &str) {
        if let Some(count) = self.open.get_mut(name) {
            *count -= 1;
            if *count == 0 {
                self.open.remove(name);
            }
        }
    }
    /** Records a capture group named `name` at `index` in the innermost scope. Fails when an open
    scope already declares the name, which covers sequences and groups nested in one another. */
    pub(super) fn declare(&mut self, name: String, index: usize) -> Result<(), PatternError> {
        if self.open.contains_key(&name) {
            return Err(PatternError("Duplicate capture group name".into()));
        }
        self.claim(&name);
        match self.slots.get(&name) {
            Some(slot) => self.entries[*slot].1.push(index),
            None => {
                self.slots.insert(name.clone(), self.entries.len());
                self.entries.push((name, vec![index]));
            }
        }
        Ok(())
    }
    /** Hands the names of a finished disjunction to the enclosing scope, where they still conflict
    with later declarations in that sequence. */
    pub(super) fn adopt(&mut self, names: Vec<String>) {
        for name in names {
            self.claim(&name);
        }
    }
    /** Registers an open claim on a name in the innermost scope. */
    fn claim(&mut self, name: &str) {
        *self.open.entry(name.to_string()).or_default() += 1;
        if let Some(frame) = self.frames.last_mut() {
            frame.push(name.to_string());
        }
    }
}
