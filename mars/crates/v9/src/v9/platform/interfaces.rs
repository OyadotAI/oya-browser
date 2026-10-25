/**
 * Element interfaces: the members only some elements have, on per-interface prototypes the
 * way Chromium's IDL lays them out (HTMLAnchorElement.prototype has `href`, a div has none).
 * Pages feature-test members with `in`: YouTube's attribute sanitizer asks `"data" in el` and
 * replaces the bound value with an inert string when an element claims a member it does not
 * have, which blanked every `data` and `text` binding while all elements shared one prototype.
 */
use super::dom;
use crate::v9::vm::realm::{Builder, NativeFn};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// One interface: its constructor name, the parent interface (None for HTMLElement), the tags
/// it serves, and its own members.
struct Interface {
    name: &'static str,
    parent: Option<&'static str>,
    tags: &'static [&'static str],
    /// Attribute-reflecting accessors (`reflect_get` / `reflect_set`).
    reflected: &'static [&'static str],
    /// Read-only members (`element_get`).
    getters: &'static [&'static str],
    /// Read-write members (`element_get` / `element_set`).
    accessors: &'static [&'static str],
    /// Methods (`element_method`).
    methods: &'static [&'static str],
}

/// URL decomposition members of anchors and areas.
const URL_PARTS: [&str; 10] = [
    "pathname", "host", "hostname", "protocol", "search", "hash", "origin", "port", "username",
    "password",
];
/// Constraint validation members of form-associated listed elements.
const VALIDATION: [&str; 3] = ["validity", "willValidate", "validationMessage"];
/// Constraint validation methods.
const VALIDATION_METHODS: [&str; 3] = ["checkValidity", "setCustomValidity", "reportValidity"];
/// HTMLMediaElement's methods.
const MEDIA_METHODS: [&str; 4] = ["load", "play", "pause", "canPlayType"];

/** An interface with no members of a kind left out. */
const fn interface(name: &'static str, tags: &'static [&'static str]) -> Interface {
    Interface {
        name,
        parent: None,
        tags,
        reflected: &[],
        getters: &[],
        accessors: &[],
        methods: &[],
    }
}

/// Every interface with members of its own. Tags not listed use HTMLElement.prototype.
const INTERFACES: &[Interface] = &[
    Interface {
        reflected: &["href", "name", "type", "rel", "target"],
        getters: &[
            "pathname", "host", "hostname", "protocol", "search", "hash", "origin", "port",
            "username", "password", "text",
        ],
        accessors: &["relList"],
        ..interface("HTMLAnchorElement", &["a"])
    },
    Interface {
        reflected: &["href", "alt", "rel", "target"],
        getters: &URL_PARTS,
        accessors: &["relList"],
        ..interface("HTMLAreaElement", &["area"])
    },
    Interface {
        reflected: &["href", "rel", "type", "media", "disabled"],
        getters: &["sheet"],
        accessors: &["relList"],
        ..interface("HTMLLinkElement", &["link"])
    },
    Interface {
        reflected: &["href", "target"],
        ..interface("HTMLBaseElement", &["base"])
    },
    Interface {
        reflected: &["src", "alt", "name", "width", "height"],
        ..interface("HTMLImageElement", &["img"])
    },
    Interface {
        reflected: &["src", "type", "defer", "async"],
        getters: &["text"],
        ..interface("HTMLScriptElement", &["script"])
    },
    Interface {
        reflected: &["src", "name", "width", "height"],
        accessors: &["sandbox"],
        ..interface("HTMLIFrameElement", &["iframe"])
    },
    Interface {
        reflected: &["src", "name"],
        ..interface("HTMLFrameElement", &["frame"])
    },
    Interface {
        reflected: &["src", "type", "name", "width", "height"],
        ..interface("HTMLEmbedElement", &["embed"])
    },
    Interface {
        reflected: &["data", "type", "name", "width", "height"],
        getters: &["form", "validity", "willValidate", "validationMessage"],
        methods: &VALIDATION_METHODS,
        ..interface("HTMLObjectElement", &["object"])
    },
    Interface {
        reflected: &["src"],
        methods: &MEDIA_METHODS,
        ..interface("HTMLMediaElement", &[])
    },
    Interface {
        parent: Some("HTMLMediaElement"),
        reflected: &["width", "height"],
        ..interface("HTMLVideoElement", &["video"])
    },
    Interface {
        parent: Some("HTMLMediaElement"),
        ..interface("HTMLAudioElement", &["audio"])
    },
    Interface {
        reflected: &["src", "type", "width", "height"],
        ..interface("HTMLSourceElement", &["source"])
    },
    Interface {
        reflected: &["src"],
        ..interface("HTMLTrackElement", &["track"])
    },
    Interface {
        reflected: &["width", "height"],
        ..interface("HTMLCanvasElement", &["canvas"])
    },
    Interface {
        reflected: &[
            "name",
            "type",
            "alt",
            "placeholder",
            "checked",
            "disabled",
            "readOnly",
            "required",
            "multiple",
            "src",
            "width",
            "height",
            "maxLength",
        ],
        getters: &["form", "validity", "willValidate", "validationMessage"],
        accessors: &["value"],
        methods: &[
            "select",
            "checkValidity",
            "setCustomValidity",
            "reportValidity",
        ],
        ..interface("HTMLInputElement", &["input"])
    },
    Interface {
        reflected: &[
            "name",
            "placeholder",
            "disabled",
            "readOnly",
            "required",
            "maxLength",
        ],
        getters: &["form", "validity", "willValidate", "validationMessage"],
        accessors: &["value"],
        methods: &[
            "select",
            "checkValidity",
            "setCustomValidity",
            "reportValidity",
        ],
        ..interface("HTMLTextAreaElement", &["textarea"])
    },
    Interface {
        reflected: &["name", "disabled", "required", "multiple"],
        getters: &[
            "form",
            "options",
            "validity",
            "willValidate",
            "validationMessage",
        ],
        accessors: &["value", "selectedIndex"],
        methods: &VALIDATION_METHODS,
        ..interface("HTMLSelectElement", &["select"])
    },
    Interface {
        reflected: &["name", "type", "disabled"],
        getters: &["form", "validity", "willValidate", "validationMessage"],
        accessors: &["value"],
        methods: &VALIDATION_METHODS,
        ..interface("HTMLButtonElement", &["button"])
    },
    Interface {
        reflected: &["selected", "disabled"],
        getters: &["form", "text"],
        accessors: &["value"],
        ..interface("HTMLOptionElement", &["option"])
    },
    Interface {
        reflected: &["disabled"],
        ..interface("HTMLOptGroupElement", &["optgroup"])
    },
    Interface {
        reflected: &["name", "disabled"],
        getters: &["form", "validity", "willValidate", "validationMessage"],
        methods: &VALIDATION_METHODS,
        ..interface("HTMLFieldSetElement", &["fieldset"])
    },
    Interface {
        reflected: &["name", "htmlFor"],
        getters: &["form", "validity", "willValidate", "validationMessage"],
        accessors: &["value"],
        methods: &VALIDATION_METHODS,
        ..interface("HTMLOutputElement", &["output"])
    },
    Interface {
        reflected: &["htmlFor"],
        getters: &["form"],
        ..interface("HTMLLabelElement", &["label"])
    },
    Interface {
        getters: &["form"],
        ..interface("HTMLLegendElement", &["legend"])
    },
    Interface {
        reflected: &["name", "rel", "target", "action"],
        accessors: &["relList"],
        methods: &[
            "submit",
            "requestSubmit",
            "reset",
            "checkValidity",
            "reportValidity",
        ],
        ..interface("HTMLFormElement", &["form"])
    },
    Interface {
        reflected: &["type", "disabled"],
        getters: &["sheet"],
        ..interface("HTMLStyleElement", &["style"])
    },
    Interface {
        reflected: &["name"],
        ..interface("HTMLMetaElement", &["meta"])
    },
    Interface {
        reflected: &["name"],
        ..interface("HTMLMapElement", &["map"])
    },
    Interface {
        reflected: &["name"],
        ..interface("HTMLSlotElement", &["slot"])
    },
    Interface {
        reflected: &["name"],
        ..interface("HTMLDetailsElement", &["details"])
    },
    Interface {
        reflected: &["name", "type"],
        accessors: &["value"],
        ..interface("HTMLParamElement", &["param"])
    },
    Interface {
        accessors: &["value"],
        ..interface("HTMLLIElement", &["li"])
    },
    Interface {
        accessors: &["value"],
        ..interface("HTMLDataElement", &["data"])
    },
    Interface {
        accessors: &["value"],
        ..interface("HTMLMeterElement", &["meter"])
    },
    Interface {
        accessors: &["value"],
        ..interface("HTMLProgressElement", &["progress"])
    },
    Interface {
        reflected: &["cite"],
        ..interface("HTMLQuoteElement", &["blockquote", "q"])
    },
    Interface {
        reflected: &["cite"],
        ..interface("HTMLModElement", &["del", "ins"])
    },
    Interface {
        reflected: &["colSpan"],
        ..interface("HTMLTableCellElement", &["td", "th"])
    },
    Interface {
        reflected: &["type"],
        ..interface("HTMLOListElement", &["ol"])
    },
    Interface {
        getters: &["text"],
        ..interface("HTMLTitleElement", &["title"])
    },
    Interface {
        getters: &["options"],
        ..interface("HTMLDataListElement", &["datalist"])
    },
    // SVG keeps the width and height it had on the shared prototype, so `svg.width` reads stay.
    Interface {
        reflected: &["width", "height"],
        ..interface("SVGSVGElement", &["svg"])
    },
];

/// Every member some interface owns; HTMLElement.prototype leaves these out.
pub(super) fn specific(name: &str) -> bool {
    INTERFACES.iter().any(|interface| {
        interface.reflected.contains(&name)
            || interface.getters.contains(&name)
            || interface.accessors.contains(&name)
            || interface.methods.contains(&name)
    }) || VALIDATION.contains(&name)
}

/// Tag to interface prototype, set once while the realm is built.
static PROTOTYPES: OnceLock<BTreeMap<&'static str, u32>> = OnceLock::new();

/** The prototype a new wrapper of an element with this tag gets, if its interface has one. */
pub(super) fn prototype_for(tag: &str) -> Option<u32> {
    let _ = crate::v9::vm::realm::realm();
    PROTOTYPES.get()?.get(tag).copied()
}

/** Build each interface's prototype, inheriting HTMLElement.prototype, and its constructor. */
pub(super) fn install(b: &mut Builder, html: u32) {
    let mut made: BTreeMap<&'static str, u32> = BTreeMap::new();
    let mut tags = BTreeMap::new();
    for interface in INTERFACES {
        let parent = interface
            .parent
            .and_then(|name| made.get(name).copied())
            .unwrap_or(html);
        let proto = b.object(Some(parent));
        members(b, proto, interface);
        let ctor = b.constructor(interface.name, 0, dom::illegal_constructor, proto, None);
        b.global(interface.name, ctor);
        made.insert(interface.name, proto);
        for tag in interface.tags {
            tags.insert(*tag, proto);
        }
    }
    let _ = PROTOTYPES.set(tags);
}

/** One interface's own members. */
fn members(b: &mut Builder, proto: u32, interface: &Interface) {
    for name in interface.reflected {
        b.accessor(proto, name, Some(dom::reflect_get), Some(dom::reflect_set));
    }
    for name in interface.getters {
        b.getter(proto, name, dom::element_get);
    }
    for name in interface.accessors {
        b.accessor(proto, name, Some(dom::element_get), Some(dom::element_set));
    }
    let method: NativeFn = dom::element_method;
    for name in interface.methods {
        b.method(proto, name, 0, method);
    }
}
