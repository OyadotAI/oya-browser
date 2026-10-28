/**
 * Installs the component interfaces into the realm: HTMLElement's constructor, the custom
 * element registry, DocumentFragment and ShadowRoot prototypes, shadow and template members on
 * elements, NodeFilter, TreeWalker, NodeIterator and MutationObserver.
 */
use super::{observer, registry, shadow, template, traversal};
use crate::v9::platform::dom;
use crate::v9::value::JsValue;
use crate::v9::vm::heap::{CONFIGURABLE, Key};
use crate::v9::vm::realm::{Builder, NativeFn, Static, string, symbol};
use crate::v9::vm::{Call, Res, Vm};
use std::sync::OnceLock;

/** Intrinsic identities the component natives use at run time. */
pub(in crate::v9) struct Ids {
    pub fragment: u32,
    pub shadow_root: u32,
    pub tree_walker: u32,
    pub node_iterator: u32,
    pub mutation_observer: u32,
    pub mutation_record: u32,
    /** The native microtask that delivers observer records. */
    pub deliver: u32,
}

/** Set once while the realm is built. */
static IDS: OnceLock<Ids> = OnceLock::new();

/** The component intrinsics (the realm is built before any page runs). */
pub(in crate::v9) fn ids() -> &'static Ids {
    let _ = crate::v9::vm::realm::realm();
    IDS.get()
        .expect("component intrinsics are installed with the realm")
}

/** Install everything; runs once, inside the realm build. */
pub(in crate::v9) fn install(b: &mut Builder) {
    let fragment = fragments(b);
    let shadow_root = shadow_roots(b, fragment);
    elements(b);
    registry_object(b);
    let (tree_walker, node_iterator) = traversals(b);
    let (mutation_observer, mutation_record) = observers(b);
    // ProcessingInstruction is installed by platform::implementation, which gives it a prototype.
    for name in ["CDATASection"] {
        // Pages patch these interfaces' prototypes; V9 creates no such nodes.
        let proto = b.object(Some(b.ids.text_prototype));
        let ctor = b.constructor(name, 0, dom::illegal_constructor, proto, None);
        b.global(name, ctor);
    }
    let deliver = b.function("", 0, observer::deliver);
    let _ = IDS.set(Ids {
        fragment,
        shadow_root,
        tree_walker,
        node_iterator,
        mutation_observer,
        mutation_record,
        deliver,
    });
}

/** Register methods that share one native dispatching on its name. */
fn methods(b: &mut Builder, target: u32, names: &[&'static str], function: NativeFn) {
    for name in names {
        b.method(target, name, 0, function);
    }
}

/** Register getters that share one native dispatching on its name. */
fn getters(b: &mut Builder, target: u32, names: &[&'static str], function: NativeFn) {
    for name in names {
        b.getter(target, name, function);
    }
}

/** DocumentFragment: parent-node members over its children, and `new DocumentFragment()`. */
fn fragments(b: &mut Builder) -> u32 {
    let proto = b.object(Some(b.ids.node_prototype));
    getters(
        b,
        proto,
        &[
            "children",
            "childElementCount",
            "firstElementChild",
            "lastElementChild",
        ],
        dom::element_get,
    );
    methods(
        b,
        proto,
        &[
            "querySelector",
            "querySelectorAll",
            "append",
            "prepend",
            "replaceChildren",
        ],
        dom::element_method,
    );
    b.method(proto, "getElementById", 1, shadow::fragment_method);
    let ctor = b.constructor("DocumentFragment", 0, construct_fragment, proto, None);
    b.global("DocumentFragment", ctor);
    proto
}

/** `new DocumentFragment()`. */
fn construct_fragment(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(
            vm.type_error("Failed to construct 'DocumentFragment': Please use the 'new' operator.")
        );
    }
    let node = vm
        .page
        .create(super::FRAGMENT, None)
        .map_err(|error| dom::dom_error(vm, error))?;
    dom::wrap(vm, node)
}

/** ShadowRoot: a fragment with a host, a mode and innerHTML. */
fn shadow_roots(b: &mut Builder, fragment: u32) -> u32 {
    let proto = b.object(Some(fragment));
    getters(
        b,
        proto,
        &[
            "host",
            "mode",
            "delegatesFocus",
            "slotAssignment",
            "clonable",
            "serializable",
            "activeElement",
        ],
        shadow::shadow_get,
    );
    b.accessor(
        proto,
        "innerHTML",
        Some(dom::element_get),
        Some(dom::element_set),
    );
    methods(
        b,
        proto,
        &[
            "getSelection",
            "elementFromPoint",
            "elementsFromPoint",
            "getAnimations",
        ],
        shadow::fragment_method,
    );
    let ctor = b.constructor("ShadowRoot", 0, dom::illegal_constructor, proto, None);
    b.global("ShadowRoot", ctor);
    proto
}

/** Shadow, slot and template members on elements; HTMLElement's constructor; getContext. */
fn elements(b: &mut Builder) {
    let element = b.ids.element_prototype;
    getters(
        b,
        element,
        &["shadowRoot", "assignedSlot"],
        shadow::element_shadow,
    );
    b.accessor(
        element,
        "slot",
        Some(dom::reflect_get),
        Some(dom::reflect_set),
    );
    let html = b.ids.html_element_prototype;
    methods(
        b,
        html,
        &["assignedNodes", "assignedElements"],
        shadow::element_shadow,
    );
    b.accessor(
        html,
        "content",
        Some(template::content_get),
        Some(template::content_set),
    );
    // A drawing context V9 cannot provide is null, as browsers answer for unavailable contexts.
    b.method(html, "getContext", 1, |_, _| Ok(JsValue::Null));
    let ctor = b.constructor(
        "HTMLElement",
        0,
        registry::html_element_construct,
        html,
        None,
    );
    b.global("HTMLElement", ctor);
}

/** `customElements` and CustomElementRegistry. */
fn registry_object(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    methods(
        b,
        proto,
        &["define", "get", "getName", "whenDefined", "upgrade"],
        registry::registry_method,
    );
    let ctor = b.constructor("CustomElementRegistry", 0, registry::illegal, proto, None);
    b.global("CustomElementRegistry", ctor);
    let registry = b.object(Some(proto));
    b.global("customElements", registry);
}

/** Symbol.toStringTag on a prototype, so `String(walker)` reads `[object TreeWalker]` as the
WebIDL interface name requires. */
fn tag_as(b: &mut Builder, target: u32, name: &'static str) {
    b.put(
        target,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string(name)),
        CONFIGURABLE,
    );
}

/** NodeFilter, TreeWalker, NodeIterator and the document factories. */
fn traversals(b: &mut Builder) -> (u32, u32) {
    let filter = b.object(Some(b.ids.object_prototype));
    for (name, value) in traversal::FILTERS {
        b.value(filter, name, JsValue::Number(value), 0);
    }
    b.global("NodeFilter", filter);
    let document = b.ids.document_prototype;
    methods(
        b,
        document,
        &["createTreeWalker", "createNodeIterator"],
        traversal::create,
    );
    let walker = b.object(Some(b.ids.object_prototype));
    getters(
        b,
        walker,
        &["root", "whatToShow", "filter"],
        traversal::walker_get,
    );
    b.accessor(
        walker,
        "currentNode",
        Some(traversal::walker_get),
        Some(traversal::walker_set),
    );
    methods(
        b,
        walker,
        &[
            "parentNode",
            "firstChild",
            "lastChild",
            "previousSibling",
            "nextSibling",
            "previousNode",
            "nextNode",
        ],
        traversal::walker_method,
    );
    tag_as(b, walker, "TreeWalker");
    let ctor = b.constructor("TreeWalker", 0, dom::illegal_constructor, walker, None);
    b.global("TreeWalker", ctor);
    let iterator = b.object(Some(b.ids.object_prototype));
    getters(
        b,
        iterator,
        &[
            "root",
            "whatToShow",
            "filter",
            "referenceNode",
            "pointerBeforeReferenceNode",
        ],
        traversal::walker_get,
    );
    methods(
        b,
        iterator,
        &["nextNode", "previousNode"],
        traversal::iterator_method,
    );
    b.method(iterator, "detach", 0, traversal::detach);
    tag_as(b, iterator, "NodeIterator");
    let ctor = b.constructor("NodeIterator", 0, dom::illegal_constructor, iterator, None);
    b.global("NodeIterator", ctor);
    (walker, iterator)
}

/** MutationObserver and MutationRecord. */
fn observers(b: &mut Builder) -> (u32, u32) {
    let proto = b.object(Some(b.ids.object_prototype));
    methods(
        b,
        proto,
        &["observe", "disconnect", "takeRecords"],
        observer::method,
    );
    let ctor = b.constructor("MutationObserver", 1, observer::construct, proto, None);
    b.global("MutationObserver", ctor);
    b.global("WebKitMutationObserver", ctor);
    let record = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("MutationRecord", 0, dom::illegal_constructor, record, None);
    b.global("MutationRecord", ctor);
    (proto, record)
}
