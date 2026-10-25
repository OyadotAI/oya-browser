/**
 * IDL [Reflect] attributes: one table per element interface, the way Blink's IDL marks each
 * attribute `[Reflect]` with its type, and one generic getter and setter per reflection type.
 * The HTML spec's reflection rules are ported here: DOMString (with LegacyNullToEmptyString),
 * nullable enumerated attributes, USVString URLs resolved against the document, enumerated
 * attributes with missing-value and invalid-value defaults, booleans, long and unsigned long
 * with the limited-to-only-non-negative, greater-than-zero, fallback and clamped rules, and
 * double (plain and limited to positive values).
 */
use super::super::{components, dom, net, scripts};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::heap::NATIVE_BASE;
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// The largest value a reflected `long` or `unsigned long` content attribute can hold.
const MAX_INT: i64 = i32::MAX as i64;
/// 2^32, for WebIDL's modular integer conversions.
const TWO_32: f64 = 4_294_967_296.0;

/// An enumerated attribute's keywords (content keyword, canonical IDL value) and defaults.
pub(super) struct Keywords {
    /// Keyword to the canonical value its state reflects as.
    pub words: &'static [(&'static str, &'static str)],
    /// The missing value default; None means no state (empty string, or null when nullable).
    pub missing: Option<&'static str>,
    /// The invalid value default; None falls back to the missing value default.
    pub invalid: Option<&'static str>,
    /// `DOMString?`: no state reads as null, and setting null removes the attribute.
    pub nullable: bool,
}

/// How one reflected attribute converts between its content value and its IDL value.
pub(super) enum Kind {
    /// DOMString, transparent and case preserving.
    Str,
    /// DOMString with [LegacyNullToEmptyString]: setting null writes "".
    NullToEmpty,
    /// USVString URL, resolved against the document base on getting.
    Url,
    /// A URL whose empty or missing value reads as the document's URL (form action).
    UrlOrDocument,
    Bool,
    Enum(&'static Keywords),
    /// long with a default.
    Long(i32),
    /// long limited to only non-negative numbers.
    LimitedLong(i32),
    /// unsigned long with a default.
    Unsigned(u32),
    /// unsigned long limited to numbers greater than zero; setting 0 throws.
    LimitedUnsigned(u32),
    /// The same, but disallowed values set the default instead of throwing.
    Fallback(u32),
    /// unsigned long clamped to [min, max]: default, min, max.
    Clamped(u32, u32, u32),
    Double(f64),
    /// double limited to positive values; setting a non-positive value is ignored.
    LimitedDouble(f64),
}

/// One reflected attribute: IDL name, content attribute ("" means the lowercased IDL name), kind.
pub(super) struct Attr {
    pub idl: &'static str,
    pub content: &'static str,
    pub kind: Kind,
}

/** A reflected attribute whose content name is its lowercased IDL name. */
const fn r(idl: &'static str, kind: Kind) -> Attr {
    Attr {
        idl,
        content: "",
        kind,
    }
}

/** A reflected attribute with a content name of its own. */
const fn rc(idl: &'static str, content: &'static str, kind: Kind) -> Attr {
    Attr { idl, content, kind }
}

const fn s(idl: &'static str) -> Attr {
    r(idl, Kind::Str)
}
const fn n(idl: &'static str) -> Attr {
    r(idl, Kind::NullToEmpty)
}
const fn b(idl: &'static str) -> Attr {
    r(idl, Kind::Bool)
}
const fn u(idl: &'static str) -> Attr {
    r(idl, Kind::Url)
}

// ---- keyword tables -----------------------------------------------------------------------

const REFERRER: Keywords = Keywords {
    words: &[
        ("", ""),
        ("no-referrer", "no-referrer"),
        ("no-referrer-when-downgrade", "no-referrer-when-downgrade"),
        ("same-origin", "same-origin"),
        ("origin", "origin"),
        ("strict-origin", "strict-origin"),
        ("origin-when-cross-origin", "origin-when-cross-origin"),
        (
            "strict-origin-when-cross-origin",
            "strict-origin-when-cross-origin",
        ),
        ("unsafe-url", "unsafe-url"),
    ],
    missing: None,
    invalid: None,
    nullable: false,
};
const CROSS_ORIGIN: Keywords = Keywords {
    words: &[
        ("anonymous", "anonymous"),
        ("use-credentials", "use-credentials"),
        ("", "anonymous"),
    ],
    missing: None,
    invalid: Some("anonymous"),
    nullable: true,
};
const ENCTYPES: [(&str, &str); 3] = [
    (
        "application/x-www-form-urlencoded",
        "application/x-www-form-urlencoded",
    ),
    ("multipart/form-data", "multipart/form-data"),
    ("text/plain", "text/plain"),
];
const ENCTYPE: Keywords = Keywords {
    words: &ENCTYPES,
    missing: Some("application/x-www-form-urlencoded"),
    invalid: None,
    nullable: false,
};
const FORM_ENCTYPE: Keywords = Keywords {
    words: &ENCTYPES,
    missing: None,
    invalid: Some("application/x-www-form-urlencoded"),
    nullable: false,
};
const METHOD: Keywords = Keywords {
    words: &[("get", "get"), ("post", "post"), ("dialog", "dialog")],
    missing: Some("get"),
    invalid: None,
    nullable: false,
};
const INPUT_FORM_METHOD: Keywords = Keywords {
    words: &[("get", "get"), ("post", "post")],
    missing: None,
    invalid: Some("get"),
    nullable: false,
};
const BUTTON_FORM_METHOD: Keywords = Keywords {
    words: &[("get", "get"), ("post", "post"), ("dialog", "dialog")],
    missing: None,
    invalid: Some("get"),
    nullable: false,
};
const ON_OFF: Keywords = Keywords {
    words: &[("on", "on"), ("off", "off")],
    missing: Some("on"),
    invalid: None,
    nullable: false,
};
const INPUT_TYPE: Keywords = Keywords {
    words: &[
        ("hidden", "hidden"),
        ("text", "text"),
        ("search", "search"),
        ("tel", "tel"),
        ("url", "url"),
        ("email", "email"),
        ("password", "password"),
        ("date", "date"),
        ("month", "month"),
        ("week", "week"),
        ("time", "time"),
        ("datetime-local", "datetime-local"),
        ("number", "number"),
        ("range", "range"),
        ("color", "color"),
        ("checkbox", "checkbox"),
        ("radio", "radio"),
        ("file", "file"),
        ("submit", "submit"),
        ("image", "image"),
        ("reset", "reset"),
        ("button", "button"),
    ],
    missing: Some("text"),
    invalid: None,
    nullable: false,
};
const BUTTON_TYPE: Keywords = Keywords {
    words: &[
        ("submit", "submit"),
        ("reset", "reset"),
        ("button", "button"),
    ],
    missing: Some("submit"),
    invalid: None,
    nullable: false,
};
const DECODING: Keywords = Keywords {
    words: &[("async", "async"), ("sync", "sync"), ("auto", "auto")],
    missing: Some("auto"),
    invalid: Some("auto"),
    nullable: false,
};
const PRELOAD: Keywords = Keywords {
    words: &[
        ("none", "none"),
        ("metadata", "metadata"),
        ("auto", "auto"),
        ("", "auto"),
    ],
    missing: Some("metadata"),
    invalid: Some("metadata"),
    nullable: false,
};
const LOADING: Keywords = Keywords {
    words: &[("lazy", "lazy"), ("eager", "eager")],
    missing: Some("eager"),
    invalid: Some("eager"),
    nullable: false,
};
const TRACK_KIND: Keywords = Keywords {
    words: &[
        ("subtitles", "subtitles"),
        ("captions", "captions"),
        ("descriptions", "descriptions"),
        ("chapters", "chapters"),
        ("metadata", "metadata"),
    ],
    missing: Some("subtitles"),
    invalid: Some("metadata"),
    nullable: false,
};
const LINK_AS: Keywords = Keywords {
    words: &[
        ("fetch", "fetch"),
        ("audio", "audio"),
        ("document", "document"),
        ("embed", "embed"),
        ("font", "font"),
        ("image", "image"),
        ("manifest", "manifest"),
        ("object", "object"),
        ("report", "report"),
        ("script", "script"),
        ("sharedworker", "sharedworker"),
        ("style", "style"),
        ("track", "track"),
        ("video", "video"),
        ("worker", "worker"),
        ("xslt", "xslt"),
    ],
    missing: None,
    invalid: None,
    nullable: false,
};
const SCOPE: Keywords = Keywords {
    words: &[
        ("row", "row"),
        ("col", "col"),
        ("rowgroup", "rowgroup"),
        ("colgroup", "colgroup"),
    ],
    missing: None,
    invalid: None,
    nullable: false,
};
const DIR: Keywords = Keywords {
    words: &[("ltr", "ltr"), ("rtl", "rtl"), ("auto", "auto")],
    missing: None,
    invalid: None,
    nullable: false,
};
const ENTER_KEY_HINT: Keywords = Keywords {
    words: &[
        ("enter", "enter"),
        ("done", "done"),
        ("go", "go"),
        ("next", "next"),
        ("previous", "previous"),
        ("search", "search"),
        ("send", "send"),
    ],
    missing: None,
    invalid: None,
    nullable: false,
};
const INPUT_MODE: Keywords = Keywords {
    words: &[
        ("none", "none"),
        ("text", "text"),
        ("tel", "tel"),
        ("url", "url"),
        ("email", "email"),
        ("numeric", "numeric"),
        ("decimal", "decimal"),
        ("search", "search"),
    ],
    missing: None,
    invalid: None,
    nullable: false,
};

// ---- per-interface tables -----------------------------------------------------------------

/// HTMLElement's own reflected attributes, on HTMLElement.prototype.
pub(super) const HTML_ELEMENT: &[Attr] = &[
    s("title"),
    s("lang"),
    r("dir", Kind::Enum(&DIR)),
    b("hidden"),
    b("autofocus"),
    b("inert"),
    rc("accessKey", "accesskey", Kind::Str),
    // ponytail: -1 for every element; focusable elements default to 0 in browsers.
    rc("tabIndex", "tabindex", Kind::Long(-1)),
    rc("enterKeyHint", "enterkeyhint", Kind::Enum(&ENTER_KEY_HINT)),
    rc("inputMode", "inputmode", Kind::Enum(&INPUT_MODE)),
];
pub(super) const ANCHOR: &[Attr] = &[
    s("target"),
    s("download"),
    s("ping"),
    s("rel"),
    s("hreflang"),
    s("type"),
    rc("referrerPolicy", "referrerpolicy", Kind::Enum(&REFERRER)),
    u("href"),
    s("coords"),
    s("charset"),
    s("name"),
    s("rev"),
    s("shape"),
];
pub(super) const AREA: &[Attr] = &[
    s("alt"),
    s("coords"),
    s("shape"),
    s("target"),
    s("download"),
    s("ping"),
    s("rel"),
    rc("referrerPolicy", "referrerpolicy", Kind::Enum(&REFERRER)),
    s("hreflang"),
    s("type"),
    u("href"),
    b("noHref"),
];
pub(super) const LINK: &[Attr] = &[
    u("href"),
    rc("crossOrigin", "crossorigin", Kind::Enum(&CROSS_ORIGIN)),
    s("rel"),
    r("as", Kind::Enum(&LINK_AS)),
    s("media"),
    s("integrity"),
    s("hreflang"),
    s("type"),
    rc("referrerPolicy", "referrerpolicy", Kind::Enum(&REFERRER)),
    s("charset"),
    s("rev"),
    s("target"),
];
pub(super) const BASE: &[Attr] = &[s("target")];
pub(super) const IMAGE: &[Attr] = &[
    s("alt"),
    u("src"),
    s("srcset"),
    rc("crossOrigin", "crossorigin", Kind::Enum(&CROSS_ORIGIN)),
    rc("useMap", "usemap", Kind::Str),
    rc("isMap", "ismap", Kind::Bool),
    r("width", Kind::Unsigned(0)),
    r("height", Kind::Unsigned(0)),
    rc("referrerPolicy", "referrerpolicy", Kind::Enum(&REFERRER)),
    r("decoding", Kind::Enum(&DECODING)),
    s("name"),
    u("lowsrc"),
    s("align"),
    r("hspace", Kind::Unsigned(0)),
    r("vspace", Kind::Unsigned(0)),
    rc("longDesc", "longdesc", Kind::Url),
    n("border"),
];
pub(super) const SCRIPT: &[Attr] = &[
    u("src"),
    s("type"),
    rc("noModule", "nomodule", Kind::Bool),
    s("charset"),
    b("defer"),
    rc("crossOrigin", "crossorigin", Kind::Enum(&CROSS_ORIGIN)),
    s("integrity"),
    s("event"),
    rc("htmlFor", "for", Kind::Str),
];
pub(super) const IFRAME: &[Attr] = &[
    u("src"),
    s("srcdoc"),
    s("name"),
    rc("allowFullscreen", "allowfullscreen", Kind::Bool),
    s("width"),
    s("height"),
    rc("referrerPolicy", "referrerpolicy", Kind::Enum(&REFERRER)),
    s("align"),
    s("scrolling"),
    rc("frameBorder", "frameborder", Kind::Str),
    rc("longDesc", "longdesc", Kind::Url),
    rc("marginHeight", "marginheight", Kind::NullToEmpty),
    rc("marginWidth", "marginwidth", Kind::NullToEmpty),
];
pub(super) const FRAME: &[Attr] = &[
    s("name"),
    s("scrolling"),
    u("src"),
    rc("frameBorder", "frameborder", Kind::Str),
    rc("longDesc", "longdesc", Kind::Url),
    rc("noResize", "noresize", Kind::Bool),
    rc("marginHeight", "marginheight", Kind::NullToEmpty),
    rc("marginWidth", "marginwidth", Kind::NullToEmpty),
];
pub(super) const EMBED: &[Attr] = &[
    u("src"),
    s("type"),
    s("width"),
    s("height"),
    s("align"),
    s("name"),
];
pub(super) const OBJECT: &[Attr] = &[
    u("data"),
    s("type"),
    s("name"),
    rc("useMap", "usemap", Kind::Str),
    s("width"),
    s("height"),
    s("align"),
    s("archive"),
    s("code"),
    b("declare"),
    r("hspace", Kind::Unsigned(0)),
    s("standby"),
    r("vspace", Kind::Unsigned(0)),
    rc("codeBase", "codebase", Kind::Url),
    rc("codeType", "codetype", Kind::Str),
    n("border"),
];
/// HTMLMediaElement's reflected attributes, shared by video and audio.
pub(super) const MEDIA: &[Attr] = &[
    u("src"),
    rc("crossOrigin", "crossorigin", Kind::Enum(&CROSS_ORIGIN)),
    r("preload", Kind::Enum(&PRELOAD)),
    b("autoplay"),
    b("loop"),
    b("controls"),
    rc("defaultMuted", "muted", Kind::Bool),
    r("loading", Kind::Enum(&LOADING)),
];
pub(super) const VIDEO: &[Attr] = &[
    r("width", Kind::Unsigned(0)),
    r("height", Kind::Unsigned(0)),
    u("poster"),
    rc("playsInline", "playsinline", Kind::Bool),
];
pub(super) const SOURCE: &[Attr] = &[
    u("src"),
    s("type"),
    s("srcset"),
    s("sizes"),
    s("media"),
    r("width", Kind::Unsigned(0)),
    r("height", Kind::Unsigned(0)),
];
pub(super) const TRACK: &[Attr] = &[
    r("kind", Kind::Enum(&TRACK_KIND)),
    u("src"),
    s("srclang"),
    s("label"),
    b("default"),
];
pub(super) const CANVAS: &[Attr] = &[
    r("width", Kind::Unsigned(300)),
    r("height", Kind::Unsigned(150)),
];
pub(super) const NAME_ONLY: &[Attr] = &[s("name")];
pub(super) const PARAM: &[Attr] = &[
    s("name"),
    s("value"),
    s("type"),
    rc("valueType", "valuetype", Kind::Str),
];
pub(super) const FORM: &[Attr] = &[
    rc("acceptCharset", "accept-charset", Kind::Str),
    r("action", Kind::UrlOrDocument),
    r("autocomplete", Kind::Enum(&ON_OFF)),
    r("enctype", Kind::Enum(&ENCTYPE)),
    rc("encoding", "enctype", Kind::Enum(&ENCTYPE)),
    r("method", Kind::Enum(&METHOD)),
    s("name"),
    rc("noValidate", "novalidate", Kind::Bool),
    s("target"),
    s("rel"),
];
pub(super) const FIELDSET: &[Attr] = &[b("disabled"), s("name")];
pub(super) const ALIGN_ONLY: &[Attr] = &[s("align")];
pub(super) const LABEL: &[Attr] = &[rc("htmlFor", "for", Kind::Str)];
pub(super) const INPUT: &[Attr] = &[
    s("accept"),
    s("alt"),
    rc("defaultChecked", "checked", Kind::Bool),
    rc("dirName", "dirname", Kind::Str),
    b("disabled"),
    rc("formAction", "formaction", Kind::UrlOrDocument),
    rc("formEnctype", "formenctype", Kind::Enum(&FORM_ENCTYPE)),
    rc("formMethod", "formmethod", Kind::Enum(&INPUT_FORM_METHOD)),
    rc("formNoValidate", "formnovalidate", Kind::Bool),
    rc("formTarget", "formtarget", Kind::Str),
    r("height", Kind::Unsigned(0)),
    s("max"),
    rc("maxLength", "maxlength", Kind::LimitedLong(-1)),
    s("min"),
    rc("minLength", "minlength", Kind::LimitedLong(-1)),
    b("multiple"),
    s("name"),
    s("pattern"),
    s("placeholder"),
    rc("readOnly", "readonly", Kind::Bool),
    b("required"),
    r("size", Kind::LimitedUnsigned(20)),
    u("src"),
    s("step"),
    r("type", Kind::Enum(&INPUT_TYPE)),
    r("width", Kind::Unsigned(0)),
    rc("defaultValue", "value", Kind::Str),
    s("align"),
    rc("useMap", "usemap", Kind::Str),
];
pub(super) const BUTTON: &[Attr] = &[
    b("disabled"),
    rc("formAction", "formaction", Kind::UrlOrDocument),
    rc("formEnctype", "formenctype", Kind::Enum(&FORM_ENCTYPE)),
    rc("formMethod", "formmethod", Kind::Enum(&BUTTON_FORM_METHOD)),
    rc("formNoValidate", "formnovalidate", Kind::Bool),
    rc("formTarget", "formtarget", Kind::Str),
    s("name"),
    r("type", Kind::Enum(&BUTTON_TYPE)),
    // `value` stays on the element accessor: input dispatch writes the typed value state.
];
pub(super) const SELECT: &[Attr] = &[
    b("disabled"),
    b("multiple"),
    s("name"),
    b("required"),
    r("size", Kind::Unsigned(0)),
];
pub(super) const OPTGROUP: &[Attr] = &[b("disabled"), s("label")];
pub(super) const OPTION: &[Attr] = &[b("disabled"), rc("defaultSelected", "selected", Kind::Bool)];
pub(super) const TEXTAREA: &[Attr] = &[
    r("cols", Kind::Fallback(20)),
    rc("dirName", "dirname", Kind::Str),
    b("disabled"),
    rc("maxLength", "maxlength", Kind::LimitedLong(-1)),
    rc("minLength", "minlength", Kind::LimitedLong(-1)),
    s("name"),
    s("placeholder"),
    rc("readOnly", "readonly", Kind::Bool),
    b("required"),
    r("rows", Kind::Fallback(2)),
    s("wrap"),
];
pub(super) const OUTPUT: &[Attr] = &[s("name")];
pub(super) const PROGRESS: &[Attr] = &[r("max", Kind::LimitedDouble(1.0))];
/// ponytail: plain reflection; the spec getters clamp min <= low <= high <= max. Clamp in a
/// custom getter when a page relies on it.
pub(super) const METER: &[Attr] = &[
    r("value", Kind::Double(0.0)),
    r("min", Kind::Double(0.0)),
    r("max", Kind::Double(1.0)),
    r("low", Kind::Double(0.0)),
    r("high", Kind::Double(1.0)),
    r("optimum", Kind::Double(0.5)),
];
pub(super) const META: &[Attr] = &[
    s("name"),
    rc("httpEquiv", "http-equiv", Kind::Str),
    s("content"),
    s("media"),
    s("scheme"),
];
pub(super) const STYLE: &[Attr] = &[s("media"), s("type")];
pub(super) const CITE: &[Attr] = &[u("cite")];
pub(super) const MOD: &[Attr] = &[u("cite"), rc("dateTime", "datetime", Kind::Str)];
pub(super) const OPEN: &[Attr] = &[b("open")];
pub(super) const DETAILS: &[Attr] = &[s("name"), b("open")];
pub(super) const COMPACT: &[Attr] = &[b("compact")];
pub(super) const HR: &[Attr] = &[
    s("align"),
    s("color"),
    rc("noShade", "noshade", Kind::Bool),
    s("size"),
    s("width"),
];
pub(super) const PRE: &[Attr] = &[r("width", Kind::Long(0))];
pub(super) const OLIST: &[Attr] = &[
    b("reversed"),
    r("start", Kind::Long(1)),
    s("type"),
    b("compact"),
];
pub(super) const ULIST: &[Attr] = &[b("compact"), s("type")];
pub(super) const LI: &[Attr] = &[r("value", Kind::Long(0)), s("type")];
pub(super) const DATA: &[Attr] = &[s("value")];
pub(super) const TIME: &[Attr] = &[rc("dateTime", "datetime", Kind::Str)];
pub(super) const BR: &[Attr] = &[s("clear")];
pub(super) const FONT: &[Attr] = &[n("color"), s("face"), s("size")];
pub(super) const FRAMESET: &[Attr] = &[s("cols"), s("rows")];
pub(super) const MARQUEE: &[Attr] = &[
    rc("bgColor", "bgcolor", Kind::Str),
    s("height"),
    r("hspace", Kind::Unsigned(0)),
    rc("scrollAmount", "scrollamount", Kind::Unsigned(6)),
    rc("scrollDelay", "scrolldelay", Kind::Unsigned(85)),
    rc("trueSpeed", "truespeed", Kind::Bool),
    r("vspace", Kind::Unsigned(0)),
    s("width"),
];
pub(super) const TABLE: &[Attr] = &[
    s("align"),
    s("border"),
    s("frame"),
    s("rules"),
    s("summary"),
    s("width"),
    rc("bgColor", "bgcolor", Kind::NullToEmpty),
    rc("cellPadding", "cellpadding", Kind::NullToEmpty),
    rc("cellSpacing", "cellspacing", Kind::NullToEmpty),
];
pub(super) const TABLE_COL: &[Attr] = &[
    r("span", Kind::Clamped(1, 1, 1000)),
    s("align"),
    rc("ch", "char", Kind::Str),
    rc("chOff", "charoff", Kind::Str),
    rc("vAlign", "valign", Kind::Str),
    s("width"),
];
pub(super) const TABLE_SECTION: &[Attr] = &[
    s("align"),
    rc("ch", "char", Kind::Str),
    rc("chOff", "charoff", Kind::Str),
    rc("vAlign", "valign", Kind::Str),
];
pub(super) const TABLE_ROW: &[Attr] = &[
    s("align"),
    rc("ch", "char", Kind::Str),
    rc("chOff", "charoff", Kind::Str),
    rc("vAlign", "valign", Kind::Str),
    rc("bgColor", "bgcolor", Kind::NullToEmpty),
];
pub(super) const TABLE_CELL: &[Attr] = &[
    rc("colSpan", "colspan", Kind::Clamped(1, 1, 1000)),
    rc("rowSpan", "rowspan", Kind::Clamped(1, 0, 65534)),
    s("headers"),
    r("scope", Kind::Enum(&SCOPE)),
    s("abbr"),
    s("align"),
    s("axis"),
    s("height"),
    s("width"),
    rc("ch", "char", Kind::Str),
    rc("chOff", "charoff", Kind::Str),
    rc("noWrap", "nowrap", Kind::Bool),
    rc("vAlign", "valign", Kind::Str),
    rc("bgColor", "bgcolor", Kind::NullToEmpty),
];

// ---- install and dispatch -----------------------------------------------------------------

/// Native function identity (getter or setter) to the attribute it reflects.
static NATIVES: OnceLock<BTreeMap<ObjectId, &'static Attr>> = OnceLock::new();

/// Collects the natives while the realm is built; `finish` publishes them.
#[derive(Default)]
pub(super) struct Registry(BTreeMap<ObjectId, &'static Attr>);

impl Registry {
    /** Define every attribute in `table` as an accessor on `proto`. */
    pub fn define(&mut self, b: &mut Builder, proto: ObjectId, table: &'static [Attr]) {
        for attr in table {
            // The accessor's getter and setter are the next two natives the builder makes.
            let get = NATIVE_BASE + b.natives.len() as ObjectId;
            b.accessor(proto, attr.idl, Some(get_attr), Some(set_attr));
            let set = get + 1;
            self.0.insert(get, attr);
            self.0.insert(set, attr);
        }
    }
    /** Publish the collected natives for `get` and `set` to find. */
    pub fn finish(self) {
        let _ = NATIVES.set(self.0);
    }
}

/** The attribute behind the native being called. */
fn attr_of(call: &Call) -> Option<&'static Attr> {
    NATIVES.get()?.get(&call.callee).copied()
}

/** The content attribute name, lowercased from the IDL name unless the table gives one. */
fn content_name(attr: &Attr) -> String {
    if attr.content.is_empty() {
        attr.idl.to_ascii_lowercase()
    } else {
        attr.content.to_string()
    }
}

/** The generic reflected getter. */
fn get_attr(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (Some(node), Some(attr)) = (dom::node_of(vm, &call.this), attr_of(&call)) else {
        return Ok(JsValue::Undefined);
    };
    let name = content_name(attr);
    let value = vm
        .page
        .view(node)
        .and_then(|view| view.attributes.get(&name).cloned());
    read(vm, &attr.kind, value)
}

/** The IDL value of a content value under `kind`. */
fn read(vm: &mut Vm, kind: &Kind, value: Option<String>) -> Res<JsValue> {
    let text = |text: &str| JsValue::string(text).map_err(Into::into);
    match kind {
        Kind::Str | Kind::NullToEmpty => text(&value.unwrap_or_default()),
        Kind::Bool => Ok(JsValue::Boolean(value.is_some())),
        Kind::Url => match value {
            None => text(""),
            Some(value) => text(&net::resolve(vm, &value).unwrap_or(value)),
        },
        Kind::UrlOrDocument => match value.filter(|v| !v.is_empty()) {
            None => text(&vm.page.state.href.clone()),
            Some(value) => text(&net::resolve(vm, &value).unwrap_or(value)),
        },
        Kind::Enum(keywords) => match keyword(keywords, value.as_deref()) {
            Some(word) => text(word),
            None if keywords.nullable => Ok(JsValue::Null),
            None => text(""),
        },
        Kind::Double(default) => Ok(JsValue::Number(
            value.and_then(|v| parse_float(&v)).unwrap_or(*default),
        )),
        Kind::LimitedDouble(default) => Ok(JsValue::Number(
            value
                .and_then(|v| parse_float(&v))
                .filter(|v| *v > 0.0)
                .unwrap_or(*default),
        )),
        _ => Ok(JsValue::Number(integer(kind, value.as_deref()) as f64)),
    }
}

/** The state an enumerated content value is in, as its canonical keyword. */
fn keyword(keywords: &Keywords, value: Option<&str>) -> Option<&'static str> {
    let Some(value) = value else {
        return keywords.missing;
    };
    keywords
        .words
        .iter()
        .find(|(word, _)| word.eq_ignore_ascii_case(value))
        .map(|(_, canonical)| *canonical)
        .or(keywords.invalid)
        .or(keywords.missing)
}

/** An integer kind's IDL value for a content value, with its default for out-of-range. */
fn integer(kind: &Kind, value: Option<&str>) -> i64 {
    let parsed = value.and_then(parse_int);
    let within = |low: i64, default: i64| {
        parsed
            .filter(|v| (low..=MAX_INT).contains(v))
            .unwrap_or(default)
    };
    match *kind {
        Kind::Long(default) => parsed
            .filter(|v| (i64::from(i32::MIN)..=MAX_INT).contains(v))
            .unwrap_or(i64::from(default)),
        Kind::LimitedLong(default) => within(0, i64::from(default)),
        Kind::Unsigned(default) => within(0, i64::from(default)),
        Kind::LimitedUnsigned(default) | Kind::Fallback(default) => within(1, i64::from(default)),
        Kind::Clamped(default, min, max) => match parsed {
            Some(v) if v >= 0 => v.clamp(i64::from(min), i64::from(max)),
            _ => i64::from(default),
        },
        _ => 0,
    }
}

/** The generic reflected setter, with custom element reactions run after it. */
fn set_attr(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| set_steps(vm, call))
}

/** The reflected setter's steps. */
fn set_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (Some(node), Some(attr)) = (dom::node_of(vm, &call.this), attr_of(&call)) else {
        if let Some(id) = call.this.object() {
            vm.put(id, dom::native_name(&call), call.arg(0))?;
        }
        return Ok(JsValue::Undefined);
    };
    let Some(value) = write(vm, &attr.kind, &call.arg(0))? else {
        return Ok(JsValue::Undefined);
    };
    let name = content_name(attr);
    vm.page
        .set_attribute(node, &name, value.as_deref())
        .map_err(|error| dom::dom_error(vm, error))?;
    if name == "src" {
        scripts::after_insert(vm, node)?;
    } else if name == "media" {
        scripts::media_changed(vm, node);
    }
    Ok(JsValue::Undefined)
}

/** The content value an IDL assignment writes: Some(None) removes, None leaves it alone. */
fn write(vm: &mut Vm, kind: &Kind, value: &JsValue) -> Res<Option<Option<String>>> {
    let index_size = |vm: &mut Vm| {
        vm.dom_exception("IndexSizeError", "The value provided is outside the range.")
    };
    let number = |n: i64| Ok(Some(Some(n.to_string())));
    match *kind {
        Kind::Bool => Ok(Some(value.truthy().then(String::new))),
        Kind::NullToEmpty if *value == JsValue::Null => Ok(Some(Some(String::new()))),
        Kind::Enum(keywords) if keywords.nullable && value.nullish() => Ok(Some(None)),
        Kind::Str | Kind::NullToEmpty | Kind::Url | Kind::UrlOrDocument | Kind::Enum(_) => {
            Ok(Some(Some(vm.to_text(value)?)))
        }
        Kind::Long(_) => number(to_int32(vm.to_number(value)?)),
        Kind::LimitedLong(_) => match to_int32(vm.to_number(value)?) {
            v if v < 0 => Err(index_size(vm)),
            v => number(v),
        },
        Kind::Unsigned(default) | Kind::Clamped(default, ..) => {
            match to_uint32(vm.to_number(value)?) {
                v if v > MAX_INT => number(i64::from(default)),
                v => number(v),
            }
        }
        Kind::LimitedUnsigned(default) => match to_uint32(vm.to_number(value)?) {
            0 => Err(index_size(vm)),
            v if v > MAX_INT => number(i64::from(default)),
            v => number(v),
        },
        Kind::Fallback(default) => match to_uint32(vm.to_number(value)?) {
            v if v == 0 || v > MAX_INT => number(i64::from(default)),
            v => number(v),
        },
        Kind::Double(_) | Kind::LimitedDouble(_) => {
            let v = vm.to_number(value)?;
            if !v.is_finite() {
                return Err(vm.type_error("The provided double value is non-finite."));
            }
            if matches!(kind, Kind::LimitedDouble(_)) && v <= 0.0 {
                return Ok(None);
            }
            Ok(Some(Some(vm.to_text(&JsValue::Number(v))?)))
        }
    }
}

/** WebIDL `long` conversion: truncate, then wrap modulo 2^32 into the signed range. */
fn to_int32(v: f64) -> i64 {
    let wrapped = to_uint32(v);
    if wrapped > MAX_INT {
        wrapped - TWO_32 as i64
    } else {
        wrapped
    }
}

/** WebIDL `unsigned long` conversion: truncate, then wrap modulo 2^32. */
fn to_uint32(v: f64) -> i64 {
    if v.is_finite() {
        v.trunc().rem_euclid(TWO_32) as i64
    } else {
        0
    }
}

// ---- the HTML spec's microsyntaxes --------------------------------------------------------

/** Skip ASCII whitespace (tab, LF, FF, CR, space), as the parsing rules do. */
fn skip_space(text: &str) -> &str {
    text.trim_start_matches(['\t', '\n', '\x0c', '\r', ' '])
}

/** The leading ASCII digits of `text`. */
fn digits(text: &str) -> &str {
    let end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    &text[..end]
}

/** Split an optional leading sign: (negative, rest). */
fn sign(text: &str) -> (bool, &str) {
    match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    }
}

/** The rules for parsing integers; saturates far outside i32 so range checks still fail. */
pub(super) fn parse_int(text: &str) -> Option<i64> {
    let (negative, rest) = sign(skip_space(text));
    let run = digits(rest);
    if run.is_empty() {
        return None;
    }
    let value = run.bytes().fold(0i64, |acc, d| {
        acc.saturating_mul(10).saturating_add(i64::from(d - b'0'))
    });
    Some(if negative { -value } else { value })
}

/** The rules for parsing floating-point number values. */
pub(super) fn parse_float(text: &str) -> Option<f64> {
    let (negative, rest) = sign(skip_space(text));
    let whole = digits(rest);
    let mut rest = &rest[whole.len()..];
    let mut fraction = "";
    if let Some(after) = rest.strip_prefix('.') {
        fraction = digits(after);
        if whole.is_empty() && fraction.is_empty() {
            return None;
        }
        rest = &after[fraction.len()..];
    } else if whole.is_empty() {
        return None;
    }
    let exponent = exponent(rest);
    let number = format!(
        "{}{}.{}e{}",
        if negative { "-" } else { "" },
        if whole.is_empty() { "0" } else { whole },
        if fraction.is_empty() { "0" } else { fraction },
        exponent
    );
    let value: f64 = number.parse().ok()?;
    // -0 parses to +0, and a value that rounds to infinity is an error.
    value
        .is_finite()
        .then_some(if value == 0.0 { 0.0 } else { value })
}

/** An `e` or `E` exponent with its sign and digits, or "0" when none follows. */
fn exponent(text: &str) -> String {
    let Some(rest) = text.strip_prefix(['e', 'E']) else {
        return "0".into();
    };
    let (negative, rest) = sign(rest);
    let run = digits(rest);
    match (run.is_empty(), negative) {
        (true, _) => "0".into(),
        (false, true) => format!("-{run}"),
        (false, false) => run.into(),
    }
}
