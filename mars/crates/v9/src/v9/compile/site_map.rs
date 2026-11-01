/**
 * The registry's map from code site to code: cheap to clone, so a page's registry travels with
 * its state from step to step, and fit for sparse keys. Sites are byte offsets times a tag count
 * over a page of tens of megabytes, so a radix trie spent most of its time allocating and freeing
 * nearly empty nodes. Here a shared hash map holds everything filed before, and a small private
 * one holds what was filed since; the small one merges into the shared one past MERGE_AT
 * entries, in place when no other state shares it.
 */
use super::Code;
use crate::v9::vm::heap::FastMap;
use std::sync::Arc;

/** Recent entries a map holds apart before merging them into its shared part: a clone copies
at most this many. */
const MERGE_AT: usize = 1024;

/** Code by site. */
#[derive(Clone, Default)]
pub(in crate::v9) struct SiteMap {
    /// Everything filed before the last merge, shared by every clone.
    shared: Arc<FastMap<u32, Arc<Code>>>,
    /// Filed since, this map's alone; it answers first.
    recent: FastMap<u32, Arc<Code>>,
}

impl SiteMap {
    /** The code at `site`. */
    pub fn get(&self, site: &u32) -> Option<&Arc<Code>> {
        self.recent.get(site).or_else(|| self.shared.get(site))
    }
    /** Whether a code is filed at `site`. */
    pub fn contains_key(&self, site: &u32) -> bool {
        self.get(site).is_some()
    }
    /** File `code` at `site`, replacing what was there. */
    pub fn insert(&mut self, site: u32, code: Arc<Code>) {
        self.recent.insert(site, code);
        if self.recent.len() >= MERGE_AT {
            self.merge();
        }
    }
    /** Move the recent entries into the shared map: in place when this map alone holds it. */
    fn merge(&mut self) {
        let recent = std::mem::take(&mut self.recent);
        Arc::make_mut(&mut self.shared).extend(recent);
    }
    /** Codes filed. */
    pub fn len(&self) -> usize {
        self.shared.len()
            + self
                .recent
                .keys()
                .filter(|site| !self.shared.contains_key(site))
                .count()
    }
    /** Every filed code, each site once. */
    pub fn values(&self) -> impl Iterator<Item = &Arc<Code>> {
        self.shared
            .iter()
            .filter(|(site, _)| !self.recent.contains_key(site))
            .map(|(_, code)| code)
            .chain(self.recent.values())
    }
    /** Keep only the codes `keep` accepts. */
    pub fn retain(&mut self, mut keep: impl FnMut(&u32, &Arc<Code>) -> bool) {
        self.merge();
        Arc::make_mut(&mut self.shared).retain(|site, code| keep(site, code));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A code of a given site, for filing. */
    fn code(site: u32) -> Arc<Code> {
        let program = crate::v9::parse_at("1", 0, Some(0)).unwrap();
        let unit = super::super::compile_script(&program, None).unwrap();
        let unit = Arc::try_unwrap(unit).unwrap_or_else(|_| unreachable!("sole owner"));
        let mut code = Arc::try_unwrap(unit.entry).unwrap_or_else(|_| unreachable!("sole owner"));
        code.site = site;
        Arc::new(code)
    }

    /** Clones share what was filed before them and diverge after; a merge keeps every entry and
    the newest code per site; retain drops exactly what it rejects. */
    #[test]
    fn clones_diverge_and_merges_keep_the_newest_code() {
        let mut map = SiteMap::default();
        for site in 0..(MERGE_AT as u32 + 10) {
            map.insert(site * 5, code(site * 5));
        }
        let before = map.clone();
        map.insert(7, code(7));
        assert!(map.contains_key(&7) && !before.contains_key(&7));
        assert_eq!(map.len(), before.len() + 1);
        let replaced = code(0);
        map.insert(0, replaced.clone());
        assert!(Arc::ptr_eq(map.get(&0).unwrap(), &replaced));
        assert!(!Arc::ptr_eq(before.get(&0).unwrap(), &replaced));
        assert_eq!(map.values().count(), map.len());
        map.retain(|site, _| site % 10 == 0);
        assert!(map.get(&5).is_none() && map.get(&10).is_some());
        assert!(before.get(&5).is_some());
    }
}
