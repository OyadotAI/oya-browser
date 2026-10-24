/**
 * Compiled code shared across the requests of one V9 process. A page's lifecycle is many
 * requests (load, each event, each timer run, each click), and each starts a fresh VM over the
 * committed state; compiling the page's megabytes of script again for every one cost more than
 * running them. A unit is keyed by a SHA-256 of its grammar, page offset and exact source text,
 * so a request can only receive the code its own text produces at that site, never another
 * page's. The bound is CODE_CACHE_BYTES of code; the least recently used unit goes first.
 */
use super::{Code, ModuleInfo};
use crate::constants::CODE_CACHE_BYTES;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

/** Which grammar the text compiled under: the same text is different code in each. */
#[derive(Clone, Copy)]
pub(super) enum Grammar {
    Script,
    Module,
}

/** One compiled source: its nested codes, its entry code and, for a module, its link facts. */
pub(super) struct Unit {
    /// Every function the source contains.
    pub codes: Vec<Arc<Code>>,
    /// The script's top-level code or the module's body.
    pub entry: Arc<Code>,
    /// Imports and exports, for a module.
    pub info: Option<Arc<ModuleInfo>>,
    /// Bytes the unit holds, charged against the bound.
    bytes: usize,
}

impl Unit {
    /** A unit over freshly compiled codes. */
    pub fn new(codes: Vec<Code>, entry: Code, info: Option<ModuleInfo>) -> Self {
        let bytes = codes.iter().chain([&entry]).map(Code::footprint).sum();
        Self {
            codes: codes.into_iter().map(Arc::new).collect(),
            entry: Arc::new(entry),
            info: info.map(Arc::new),
            bytes,
        }
    }
}

/** SHA-256 of grammar, offset and source. */
pub(super) type Key = [u8; 32];

/** The process cache: units by key, with use order and the bytes they hold. */
#[derive(Default)]
struct Cache {
    units: HashMap<Key, Arc<Unit>>,
    /// Least recently used first.
    order: VecDeque<Key>,
    bytes: usize,
}

/** The one cache of this process. */
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

/** The cache, usable even after a panicking holder. */
fn cache() -> std::sync::MutexGuard<'static, Cache> {
    CACHE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/** The key of `source` compiled under `grammar` at page offset `offset`. */
pub(super) fn key(grammar: Grammar, offset: usize, source: &str) -> Key {
    let mut context = ring::digest::Context::new(&ring::digest::SHA256);
    context.update(&[match grammar {
        Grammar::Script => 0,
        Grammar::Module => 1,
    }]);
    context.update(&(offset as u64).to_le_bytes());
    context.update(&(source.len() as u64).to_le_bytes());
    context.update(source.as_bytes());
    let mut key = Key::default();
    key.copy_from_slice(context.finish().as_ref());
    key
}

/** The unit under `key`, now the most recently used. */
pub(super) fn get(key: &Key) -> Option<Arc<Unit>> {
    let mut cache = cache();
    let unit = cache.units.get(key)?.clone();
    if let Some(position) = cache.order.iter().position(|known| known == key) {
        cache.order.remove(position);
    }
    cache.order.push_back(*key);
    Some(unit)
}

/** Keeps `unit` under `key`, evicting the least recently used units past the bound; a unit
larger than the whole bound is not kept. */
pub(super) fn put(key: Key, unit: Arc<Unit>) {
    if unit.bytes > CODE_CACHE_BYTES {
        return;
    }
    let mut cache = cache();
    if cache.units.contains_key(&key) {
        return;
    }
    while cache.bytes + unit.bytes > CODE_CACHE_BYTES {
        let Some(oldest) = cache.order.pop_front() else {
            break;
        };
        if let Some(evicted) = cache.units.remove(&oldest) {
            cache.bytes -= evicted.bytes;
        }
    }
    cache.bytes += unit.bytes;
    cache.order.push_back(key);
    cache.units.insert(key, unit);
}

/** Compiled units cross threads: every request runs on its own stack. */
#[allow(dead_code)]
fn units_are_shareable() {
    fn shareable<T: Send + Sync>() {}
    shareable::<Unit>();
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Keys separate grammar, offset and text, including texts that differ only in length. */
    #[test]
    fn keys_separate_grammar_offset_and_text() {
        let script = key(Grammar::Script, 0, "a");
        assert_ne!(script, key(Grammar::Module, 0, "a"));
        assert_ne!(script, key(Grammar::Script, 1, "a"));
        assert_ne!(script, key(Grammar::Script, 0, "b"));
        assert_eq!(script, key(Grammar::Script, 0, "a"));
    }
}
