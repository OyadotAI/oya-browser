/** SVG wrappers select the already-installed SVG interface without borrowing HTMLElement prototypes. */
use super::super::chrome_shapes::Known;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/** Actual SVG local names are case-sensitive; unknown names retain the SVGElement base interface. */
const TAGS: &[&str] = &[
    "a",
    "animate",
    "animateMotion",
    "animateTransform",
    "circle",
    "clipPath",
    "defs",
    "desc",
    "ellipse",
    "feBlend",
    "feColorMatrix",
    "feComponentTransfer",
    "feComposite",
    "feConvolveMatrix",
    "feDiffuseLighting",
    "feDisplacementMap",
    "feDistantLight",
    "feDropShadow",
    "feFlood",
    "feFuncA",
    "feFuncB",
    "feFuncG",
    "feFuncR",
    "feGaussianBlur",
    "feImage",
    "feMerge",
    "feMergeNode",
    "feMorphology",
    "feOffset",
    "fePointLight",
    "feSpecularLighting",
    "feSpotLight",
    "feTile",
    "feTurbulence",
    "filter",
    "foreignObject",
    "g",
    "image",
    "line",
    "linearGradient",
    "marker",
    "mask",
    "metadata",
    "mpath",
    "path",
    "pattern",
    "polygon",
    "polyline",
    "radialGradient",
    "rect",
    "script",
    "set",
    "stop",
    "style",
    "svg",
    "switch",
    "symbol",
    "text",
    "textPath",
    "title",
    "tspan",
    "use",
    "view",
];

/** The empty key holds the base prototype for SVG names without a specialized interface. */
static PROTOTYPES: OnceLock<BTreeMap<&'static str, u32>> = OnceLock::new();

/** The shape installer has resolved every parent and preserves concrete SVGSVGElement bindings. */
pub(in crate::v9::platform) fn install_shapes(known: &Known) {
    let mut prototypes = BTreeMap::new();
    if let Some((_, prototype)) = known.get("SVGElement") {
        prototypes.insert("", *prototype);
    }
    for tag in TAGS {
        let name = interface_name(tag);
        if let Some((_, prototype)) = known.get(name.as_str()) {
            prototypes.insert(*tag, *prototype);
        }
    }
    let _ = PROTOTYPES.set(prototypes);
}

/** WebIDL constructor spellings have only three special acronym cases beyond the filter prefix. */
fn interface_name(tag: &str) -> String {
    let body = match tag {
        "svg" => "SVG".into(),
        "mpath" => "MPath".into(),
        "tspan" => "TSpan".into(),
        _ if tag.starts_with("fe") => format!("FE{}", &tag[2..]),
        _ => format!("{}{}", tag[..1].to_ascii_uppercase(), &tag[1..]),
    };
    format!("SVG{body}Element")
}

/** Only an already-established SVG namespace uses this registry. */
pub(in crate::v9::platform) fn prototype_for(local: &str) -> Option<u32> {
    let prototypes = PROTOTYPES.get()?;
    prototypes
        .get(local)
        .or_else(|| prototypes.get(""))
        .copied()
}
