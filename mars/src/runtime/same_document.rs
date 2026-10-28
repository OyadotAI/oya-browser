/**
 * Same-document history: entries a page adds with pushState or a hash change, and Back and
 * Forward between them. A single-page app (React Router, Next.js) routes this way; without it Back
 * reloaded the whole app from the network and lost its in-memory state, and pushState never
 * added an entry, so Back left the app entirely.
 */
use super::navigation::HistoryEntry;
use super::supervisor::Runtime;
use crate::constants::HISTORY_ENTRIES;
use crate::protocol::EngineResult;
use serde_json::{Value, json};

impl Runtime {
    /** Folds the committed page's queued history records into the tab's history: a push adds an
    entry after the current one (dropping the forward entries, as a new navigation does) and a
    replace rewrites the current entry. Both stay with the current document. */
    pub(super) fn adopt_history_records(&mut self) {
        let Some(page) = self.page.as_mut() else {
            return;
        };
        let records = std::mem::take(&mut page.scripting.history_records);
        let (sensitive, document) = (page.sensitive, page.generation);
        for record in records {
            let entry = HistoryEntry {
                url: record.url,
                sensitive,
                replayable: true,
                document,
                state: record.state,
            };
            self.record_entry(entry, record.replace);
        }
    }

    /** One same-document entry: replaces the current one or is pushed after it. */
    fn record_entry(&mut self, entry: HistoryEntry, replace: bool) {
        let Some(index) = self.history_cursor else {
            return;
        };
        if replace {
            self.history[index] = entry;
            return;
        }
        self.history.truncate(index + 1);
        self.history.push(entry);
        if self.history.len() > HISTORY_ENTRIES {
            self.history.remove(0);
        }
        self.history_cursor = Some(self.history.len() - 1);
    }

    /** Whether the entry at `index` is another entry of the document on screen. The current
    entry itself never is: Reload revisits it and must load a fresh document. */
    pub(super) fn same_document(&self, index: usize) -> bool {
        if self.history_cursor == Some(index) {
            return false;
        }
        let current = self.history_cursor.and_then(|at| self.history.get(at));
        let target = self.history.get(index);
        let page = self.page.as_ref();
        match (current, target, page) {
            (Some(current), Some(target), Some(page)) => {
                current.document == target.document && target.document == page.generation
            }
            _ => false,
        }
    }

    /** Back or Forward within one document: the URL changes in place and the page's scripts get
    `popstate` (and `hashchange` for a fragment move), with no request and no reload. The cursor
    moves first so the commit does not rewrite the entry being left. */
    pub(super) fn traverse_same_document(&mut self, index: usize) -> EngineResult<Value> {
        let mut page = self.current()?.clone();
        let entry = self.history[index].clone();
        let hash_changed = fragment_move(&page.url, &entry.url);
        page.url.clone_from(&entry.url);
        let previous = self.history_cursor.replace(index);
        let result = self.pop_state(page, entry.state, hash_changed);
        if result.is_err() {
            self.history_cursor = previous;
        }
        result
    }

    /** Runs `popstate` in the page, lets it settle like a click, and commits what it changed. */
    fn pop_state(
        &mut self,
        mut page: super::page::Page,
        state: Option<String>,
        hash_changed: bool,
    ) -> EngineResult<Value> {
        if !page.scripting.loaded {
            return self.finish_mutation(page);
        }
        let event = json!({"pop_state":{"state":state,"hash_changed":hash_changed}});
        self.script_step(&mut page, json!({"kind":"event","event":event}))?;
        let deadline = std::time::Instant::now() + crate::constants::WEB_TIMEOUT;
        self.settle_after_load(&mut page, deadline)?;
        page.scripting.wake();
        self.finish_mutation(page)
    }
}

/** Two URLs of one document that differ only in their fragment. */
fn fragment_move(from: &str, to: &str) -> bool {
    let strip = |url: &str| url.split_once('#').map_or(url, |(base, _)| base).to_owned();
    from != to && strip(from) == strip(to)
}

#[cfg(test)]
mod tests {
    use super::fragment_move;

    /** Only a fragment-only move between distinct URLs fires hashchange. */
    #[test]
    fn a_fragment_only_move_is_a_hash_change() {
        assert!(fragment_move("https://a.test/x#1", "https://a.test/x#2"));
        assert!(fragment_move("https://a.test/x", "https://a.test/x#2"));
        assert!(!fragment_move("https://a.test/x#1", "https://a.test/y#1"));
        assert!(!fragment_move("https://a.test/x#1", "https://a.test/x#1"));
    }
}
