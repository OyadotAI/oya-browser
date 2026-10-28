/**
 * Live HTMLCollection lists: getElementsByTagName, getElementsByTagNameNS and getElementsByClassName.
 * A list keeps its last answer together with the tree generation it was read at. A read runs the
 * query again only when a mutation has moved the generation on, so every read between two mutations
 * is O(1) (or O(log n) for the index) and one burst of mutations costs one O(n) rescan of the subtree.
 */
use super::dom::search;
use super::Page;
use crate::v9::value::JsValue;
use crate::v9::vm::data::{Class, HostData};
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::realm;
use crate::v9::vm::{Res, Vm};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, PoisonError};

/** The namespace every element carries in this node model, which stores no namespace records. */
pub(in crate::v9) const HTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";
/** The wildcard that matches any namespace or any local name. */
pub(in crate::v9) const WILDCARD: &str = "*";

/** Which elements a live list holds. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) enum Filter {
    /** Elements with this tag name, lowercased by the caller; `*` matches every element. */
    Tag(String),
    /** Elements whose local name matches, when `namespace_matches`: the namespace was `*` or HTML. */
    TagNs { namespace_matches: bool, local: String },
    /** Elements carrying every one of these classes; an empty set matches nothing. */
    Class(Vec<String>),
}

/** The last answer a list read, and the generation it was read at. Generation 0 is never current. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Cached {
    generation: u64,
    nodes: Arc<[usize]>,
}

/** A live list: its root, its filter and the cached answer. Shared by clones of its host handle. */
#[derive(Debug, Serialize, Deserialize)]
pub(in crate::v9) struct LiveList {
    root: usize,
    filter: Filter,
    cache: Mutex<Cached>,
}

impl Filter {
    /** Whether an element on the page belongs in the list this filter describes. */
    fn matches(&self, page: &mut Page, id: usize) -> bool {
        match self {
            Self::Tag(tag) => page.is_element(id) && (tag == WILDCARD || page.tag(id) == *tag),
            Self::TagNs {
                namespace_matches,
                local,
            } => {
                *namespace_matches
                    && page.is_element(id)
                    && (local == WILDCARD || page.tag(id) == *local)
            }
            Self::Class(wanted) => has_classes(page, id, wanted),
        }
    }
}

/** Whether an element's class attribute contains every wanted class. An empty set matches nothing,
as the DOM spec requires for getElementsByClassName(""). */
fn has_classes(page: &mut Page, id: usize, wanted: &[String]) -> bool {
    if wanted.is_empty() {
        return false;
    }
    let classes = page.attribute_value(id, "class").unwrap_or_default();
    wanted
        .iter()
        .all(|class| classes.split_ascii_whitespace().any(|c| c == class))
}

/** The filter for getElementsByTagNameNS. The null namespace, which is the empty string here, matches
no element; `*` and the HTML namespace match all of them, since the node model holds no others. */
pub(in crate::v9) fn tag_ns_filter(namespace: &str, local: &str) -> Filter {
    Filter::TagNs {
        namespace_matches: namespace == WILDCARD || namespace == HTML_NAMESPACE,
        local: local.to_string(),
    }
}

impl LiveList {
    /** A list over `root` for `filter`. It holds no answer until its first read. */
    pub fn new(root: usize, filter: Filter) -> Self {
        Self {
            root,
            filter,
            cache: Mutex::new(Cached::default()),
        }
    }
}

/** The nodes a live list holds now. The cached answer is returned while the tree generation is the
one it was read at; otherwise the subtree is scanned once and the answer kept. Returns an Arc, so a
read copies a pointer, not the list. */
pub(in crate::v9) fn current(vm: &mut Vm, list: &LiveList) -> Arc<[usize]> {
    let generation = vm.page.generation;
    let mut cached = list.cache.lock().unwrap_or_else(PoisonError::into_inner);
    if cached.generation != generation {
        let found = search(vm, list.root, |page, id| list.filter.matches(page, id));
        *cached = Cached {
            generation,
            nodes: found.into(),
        };
    }
    cached.nodes.clone()
}

/** A namespace argument. Null or undefined is the null namespace, which is the empty string here. */
pub(in crate::v9) fn namespace_arg(vm: &mut Vm, value: &JsValue) -> Res<String> {
    match value {
        JsValue::Null | JsValue::Undefined => Ok(String::new()),
        other => vm.to_text(other),
    }
}

/** A new live HTMLCollection over `root`: its reads follow the tree as it changes. */
pub(in crate::v9) fn live_collection(vm: &mut Vm, root: usize, filter: Filter) -> Res<JsValue> {
    let list = Arc::new(LiveList::new(root, filter));
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Live(list))),
        Some(realm().ids.html_collection_prototype),
    ))?;
    Ok(JsValue::Object(id))
}
