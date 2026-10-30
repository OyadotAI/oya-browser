/**
 * Element interfaces: the members only some elements have, on per-interface prototypes the
 * way Chromium's IDL lays them out (HTMLAnchorElement.prototype has `href`, a div has none).
 * Pages feature-test members with `in`: YouTube's attribute sanitizer asks `"data" in el` and
 * replaces the bound value with an inert string when an element claims a member it does not
 * have, which blanked every `data` and `text` binding while all elements shared one prototype.
 */
use super::dom;
mod svg;
pub(super) use svg::{install_shapes as install_svg_shapes, prototype_for as svg_prototype_for};
#[path = "popover.rs"]
pub(super) mod popover;
#[path = "reflect.rs"]
mod reflect;
use crate::v9::vm::realm::{Builder, NativeFn};
use reflect::Attr;
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
    /// The typed [Reflect] table; its entries replace a same-named member above.
    table: &'static [Attr],
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
        table: &[],
    }
}

/// Every interface with members of its own. Tags not listed use HTMLElement.prototype.
const INTERFACES: &[Interface] = &[
    // Its window-forwarded handler accessors are added in `install`, beside the other members.
    interface("HTMLBodyElement", &["body"]),
    interface("HTMLHeadElement", &["head"]),
    Interface {
        reflected: &["href", "name", "type", "rel", "target"],
        getters: &[
            "pathname", "host", "hostname", "protocol", "search", "hash", "origin", "port",
            "username", "password", "text",
        ],
        accessors: &["relList"],
        table: reflect::ANCHOR,
        ..interface("HTMLAnchorElement", &["a"])
    },
    Interface {
        reflected: &["href", "alt", "rel", "target"],
        getters: &URL_PARTS,
        accessors: &["relList"],
        table: reflect::AREA,
        ..interface("HTMLAreaElement", &["area"])
    },
    Interface {
        reflected: &["href", "rel", "type", "media", "disabled"],
        getters: &["sheet"],
        accessors: &["relList"],
        table: reflect::LINK,
        ..interface("HTMLLinkElement", &["link"])
    },
    Interface {
        reflected: &["href", "target"],
        table: reflect::BASE,
        ..interface("HTMLBaseElement", &["base"])
    },
    Interface {
        reflected: &["src", "alt", "name", "width", "height"],
        table: reflect::IMAGE,
        ..interface("HTMLImageElement", &["img"])
    },
    Interface {
        reflected: &["src", "type", "defer", "async"],
        getters: &["text"],
        table: reflect::SCRIPT,
        ..interface("HTMLScriptElement", &["script"])
    },
    Interface {
        reflected: &["src", "srcdoc", "name", "width", "height"],
        getters: &["contentDocument", "contentWindow"],
        accessors: &["sandbox"],
        table: reflect::IFRAME,
        ..interface("HTMLIFrameElement", &["iframe"])
    },
    Interface {
        reflected: &["src", "name"],
        table: reflect::FRAME,
        ..interface("HTMLFrameElement", &["frame"])
    },
    Interface {
        reflected: &["src", "type", "name", "width", "height"],
        table: reflect::EMBED,
        ..interface("HTMLEmbedElement", &["embed"])
    },
    Interface {
        reflected: &["data", "type", "name", "width", "height"],
        getters: &["form", "validity", "willValidate", "validationMessage"],
        methods: &VALIDATION_METHODS,
        table: reflect::OBJECT,
        ..interface("HTMLObjectElement", &["object"])
    },
    Interface {
        reflected: &["src"],
        methods: &MEDIA_METHODS,
        table: reflect::MEDIA,
        ..interface("HTMLMediaElement", &[])
    },
    Interface {
        parent: Some("HTMLMediaElement"),
        reflected: &["width", "height"],
        table: reflect::VIDEO,
        ..interface("HTMLVideoElement", &["video"])
    },
    Interface {
        parent: Some("HTMLMediaElement"),
        ..interface("HTMLAudioElement", &["audio"])
    },
    Interface {
        reflected: &["src", "type", "width", "height"],
        table: reflect::SOURCE,
        ..interface("HTMLSourceElement", &["source"])
    },
    Interface {
        reflected: &["src"],
        table: reflect::TRACK,
        ..interface("HTMLTrackElement", &["track"])
    },
    Interface {
        reflected: &["width", "height"],
        table: reflect::CANVAS,
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
        table: reflect::INPUT,
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
        table: reflect::TEXTAREA,
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
        table: reflect::SELECT,
        ..interface("HTMLSelectElement", &["select"])
    },
    Interface {
        reflected: &["name", "type", "disabled"],
        getters: &["form", "validity", "willValidate", "validationMessage"],
        accessors: &["value"],
        methods: &VALIDATION_METHODS,
        table: reflect::BUTTON,
        ..interface("HTMLButtonElement", &["button"])
    },
    Interface {
        reflected: &["selected", "disabled"],
        getters: &["form", "text"],
        accessors: &["value"],
        table: reflect::OPTION,
        ..interface("HTMLOptionElement", &["option"])
    },
    Interface {
        reflected: &["disabled"],
        table: reflect::OPTGROUP,
        ..interface("HTMLOptGroupElement", &["optgroup"])
    },
    Interface {
        reflected: &["name", "disabled"],
        getters: &["form", "validity", "willValidate", "validationMessage"],
        methods: &VALIDATION_METHODS,
        table: reflect::FIELDSET,
        ..interface("HTMLFieldSetElement", &["fieldset"])
    },
    Interface {
        reflected: &["name", "htmlFor"],
        getters: &["form", "validity", "willValidate", "validationMessage"],
        accessors: &["value"],
        methods: &VALIDATION_METHODS,
        table: reflect::OUTPUT,
        ..interface("HTMLOutputElement", &["output"])
    },
    Interface {
        reflected: &["htmlFor"],
        getters: &["form"],
        table: reflect::LABEL,
        ..interface("HTMLLabelElement", &["label"])
    },
    Interface {
        getters: &["form"],
        table: reflect::ALIGN_ONLY,
        ..interface("HTMLLegendElement", &["legend"])
    },
    Interface {
        reflected: &["name", "rel", "target", "action"],
        getters: &["elements"],
        accessors: &["relList"],
        methods: &[
            "submit",
            "requestSubmit",
            "reset",
            "checkValidity",
            "reportValidity",
        ],
        table: reflect::FORM,
        ..interface("HTMLFormElement", &["form"])
    },
    Interface {
        reflected: &["type", "disabled"],
        getters: &["sheet"],
        table: reflect::STYLE,
        ..interface("HTMLStyleElement", &["style"])
    },
    Interface {
        reflected: &["name"],
        table: reflect::META,
        ..interface("HTMLMetaElement", &["meta"])
    },
    Interface {
        reflected: &["name"],
        table: reflect::NAME_ONLY,
        ..interface("HTMLMapElement", &["map"])
    },
    Interface {
        reflected: &["name"],
        table: reflect::NAME_ONLY,
        ..interface("HTMLSlotElement", &["slot"])
    },
    Interface {
        reflected: &["name"],
        table: reflect::DETAILS,
        ..interface("HTMLDetailsElement", &["details"])
    },
    Interface {
        reflected: &["name", "type"],
        accessors: &["value"],
        table: reflect::PARAM,
        ..interface("HTMLParamElement", &["param"])
    },
    Interface {
        accessors: &["value"],
        table: reflect::LI,
        ..interface("HTMLLIElement", &["li"])
    },
    Interface {
        accessors: &["value"],
        table: reflect::DATA,
        ..interface("HTMLDataElement", &["data"])
    },
    Interface {
        accessors: &["value"],
        table: reflect::METER,
        ..interface("HTMLMeterElement", &["meter"])
    },
    Interface {
        accessors: &["value"],
        table: reflect::PROGRESS,
        ..interface("HTMLProgressElement", &["progress"])
    },
    Interface {
        reflected: &["cite"],
        table: reflect::CITE,
        ..interface("HTMLQuoteElement", &["blockquote", "q"])
    },
    Interface {
        reflected: &["cite"],
        table: reflect::MOD,
        ..interface("HTMLModElement", &["del", "ins"])
    },
    Interface {
        reflected: &["colSpan"],
        table: reflect::TABLE_CELL,
        ..interface("HTMLTableCellElement", &["td", "th"])
    },
    Interface {
        reflected: &["type"],
        table: reflect::OLIST,
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
    Interface {
        table: reflect::ALIGN_ONLY,
        ..interface("HTMLParagraphElement", &["p"])
    },
    Interface {
        table: reflect::HR,
        ..interface("HTMLHRElement", &["hr"])
    },
    Interface {
        table: reflect::PRE,
        ..interface("HTMLPreElement", &["pre", "listing", "xmp"])
    },
    Interface {
        table: reflect::ULIST,
        ..interface("HTMLUListElement", &["ul"])
    },
    Interface {
        table: reflect::COMPACT,
        ..interface("HTMLDListElement", &["dl"])
    },
    Interface {
        table: reflect::COMPACT,
        ..interface("HTMLMenuElement", &["menu"])
    },
    Interface {
        table: reflect::COMPACT,
        ..interface("HTMLDirectoryElement", &["dir"])
    },
    Interface {
        table: reflect::OPEN,
        ..interface("HTMLDialogElement", &["dialog"])
    },
    Interface {
        table: reflect::ALIGN_ONLY,
        ..interface("HTMLHeadingElement", &["h1", "h2", "h3", "h4", "h5", "h6"])
    },
    Interface {
        table: reflect::BR,
        ..interface("HTMLBRElement", &["br"])
    },
    Interface {
        table: reflect::TIME,
        ..interface("HTMLTimeElement", &["time"])
    },
    Interface {
        table: reflect::FONT,
        ..interface("HTMLFontElement", &["font"])
    },
    Interface {
        table: reflect::FRAMESET,
        ..interface("HTMLFrameSetElement", &["frameset"])
    },
    Interface {
        table: reflect::MARQUEE,
        ..interface("HTMLMarqueeElement", &["marquee"])
    },
    Interface {
        table: reflect::TABLE,
        ..interface("HTMLTableElement", &["table"])
    },
    Interface {
        table: reflect::ALIGN_ONLY,
        ..interface("HTMLTableCaptionElement", &["caption"])
    },
    Interface {
        table: reflect::TABLE_COL,
        ..interface("HTMLTableColElement", &["colgroup", "col"])
    },
    Interface {
        table: reflect::TABLE_SECTION,
        ..interface("HTMLTableSectionElement", &["tbody", "thead", "tfoot"])
    },
    Interface {
        table: reflect::TABLE_ROW,
        ..interface("HTMLTableRowElement", &["tr"])
    },
    // SVG prototypes must never alias HTMLElement: shape completion adds readonly SVG members.
    Interface {
        parent: Some("Element"),
        methods: &["focus", "blur"],
        ..interface("SVGElement", &[])
    },
    Interface {
        parent: Some("SVGElement"),
        ..interface("SVGGraphicsElement", &[])
    },
    Interface {
        parent: Some("SVGGraphicsElement"),
        reflected: &["width", "height"],
        ..interface("SVGSVGElement", &[])
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
    let mut made: BTreeMap<&'static str, u32> =
        BTreeMap::from([("Element", b.ids.element_prototype)]);
    let mut tags = BTreeMap::new();
    let mut registry = reflect::Registry::default();
    registry.define(b, html, reflect::HTML_ELEMENT);
    for interface in INTERFACES {
        let parent = interface
            .parent
            .and_then(|name| made.get(name).copied())
            .unwrap_or(html);
        let proto = b.object(Some(parent));
        members(b, proto, interface);
        if interface.name == "HTMLImageElement" {
            super::images::install(b, proto);
        }
        if interface.name == "HTMLBodyElement" {
            super::events::install_body_handlers(b, proto);
        }
        registry.define(b, proto, interface.table);
        if interface.name == "HTMLCanvasElement" {
            super::canvas::install(b, proto);
        }
        let ctor = b.constructor(interface.name, 0, dom::illegal_constructor, proto, None);
        b.global(interface.name, ctor);
        made.insert(interface.name, proto);
        for tag in interface.tags {
            tags.insert(*tag, proto);
        }
    }
    let invokers = ["HTMLButtonElement", "HTMLInputElement"].map(|name| made[name]);
    popover::install(b, html, &invokers);
    let _ = PROTOTYPES.set(tags);
    registry.finish();
    super::blob::install(b);
    super::form_data::install(b);
    super::encoding::install(b);
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
        b.host_method(proto, name, 0, method);
    }
}
