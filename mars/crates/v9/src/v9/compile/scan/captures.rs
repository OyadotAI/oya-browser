/** Memoized lexical dependencies keep unrelated function locals out of retained closure environments. */
use super::{Body, Func, HashSet, Name, Walker};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/** Each syntax function is analyzed once, including functions compiled lazily on later tasks. */
#[derive(Clone, Default)]
pub(in crate::v9::compile) struct Cache(Arc<Mutex<HashMap<u32, Arc<Summary>>>>);

/** Free references propagate outward until a function binds them; nested references require capture. */
pub(in crate::v9::compile) struct Summary {
    free: HashSet<Name>,
    pub captured: Arc<HashSet<Name>>,
    pub dynamic: bool,
}

/** One function's syntax traversal, with nested functions resolved through the shared cache. */
pub(super) struct Walk {
    cache: Cache,
    free: HashSet<Name>,
    captured: HashSet<Name>,
    pub dynamic: bool,
}

impl Cache {
    /** Reuse a function's dependencies without rescanning its nested syntax at each lazy compile. */
    pub(in crate::v9::compile) fn analyze(&self, func: &Func) -> Arc<Summary> {
        if let Some(summary) = self.0.lock().unwrap().get(&func.site).cloned() {
            return summary;
        }
        let mut walker = Walker {
            capture: Some(Walk::new(self.clone())),
            ..Walker::default()
        };
        walker.function_contents(func);
        let mut walk = walker.capture.unwrap();
        for name in local_names(func) {
            walk.free.remove(&name);
        }
        let summary = Arc::new(Summary {
            free: walk.free,
            captured: Arc::new(walk.captured),
            dynamic: walk.dynamic,
        });
        self.0
            .lock()
            .unwrap()
            .entry(func.site)
            .or_insert(summary)
            .clone()
    }
}

impl Walk {
    /** Start with no references; unknown direct-eval names are recorded separately. */
    fn new(cache: Cache) -> Self {
        Self {
            cache,
            free: HashSet::new(),
            captured: HashSet::new(),
            dynamic: false,
        }
    }
    /** A direct reference is free until the function's own declarations are removed. */
    pub(super) fn name(&mut self, name: &Name) {
        self.free.insert(name.clone());
    }
    /** Only free child references need bindings from this function or one of its ancestors. */
    pub(super) fn nested(&mut self, func: &Func) {
        let summary = self.cache.analyze(func);
        self.free.extend(summary.free.iter().cloned());
        self.captured.extend(summary.free.iter().cloned());
        self.dynamic |= summary.dynamic;
    }
}

/** Function-wide bindings stop references from escaping; block-local shadows stay conservative. */
fn local_names(func: &Func) -> Vec<Name> {
    let mut names = Vec::new();
    for param in &func.params {
        super::bound_names(param, &mut names);
    }
    if let Some(rest) = &func.rest {
        super::bound_names(rest, &mut names);
    }
    if let Body::Block(body) = &func.body {
        super::var_names(body, func.strict, true, &mut names);
        names.extend(
            super::lexical_names(body, false)
                .into_iter()
                .map(|(name, _)| name),
        );
        names.extend(
            super::function_declarations(body)
                .iter()
                .filter_map(|func| func.name.clone()),
        );
    }
    if !func.flags.arrow {
        names.extend(
            ["this", "arguments", "new.target", "%home"]
                .into_iter()
                .map(Name::from),
        );
    }
    names
}
