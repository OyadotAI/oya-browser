use super::attr_dom;
use super::components;
/** The DOM scripts see: node wrappers, the document, and Node/Element/HTMLElement members. */
pub(in crate::v9) use super::html::{parse_detached, serialize};
use super::implementation::{self, PROCESSING_INSTRUCTION};
use super::namespaces::{self, ElementNs, HTML_NAMESPACE, NameError, NameKind};
use super::nodes::{BODY, HEAD, HTML, VIRTUAL_BASE};
use super::range_live;
use super::selector;
use super::token_list;
use super::token_set::TokenSet;
use crate::document::InsertPosition;
use crate::protocol::EngineError;
use crate::v9::value::{JsString, JsValue, ObjectId};
use crate::v9::vm::data::{Class, HostData};
use crate::v9::vm::heap::{Key, Object, Prop, Slot};
use crate::v9::vm::realm::realm;
use crate::v9::vm::{Call, Res, Thrown, Vm};
use mars_core::charset::{self, encode_url_query};
use mars_core::http::UTF_8;

/** The size V9 reports for a rendered element's box, since it computes no layout. */
const NOMINAL_BOX: f64 = 1.0;

/** The document as a node identity for traversal. */
pub(in crate::v9) const DOCUMENT: usize = VIRTUAL_BASE + 3;

/** Engine errors from the node model become script errors where scripts can see them. */
pub(in crate::v9) fn dom_error(vm: &mut Vm, error: EngineError) -> Thrown {
    match error.code.as_str() {
        "unsupported_feature" => {
            let message = error.message.clone();
            vm.unsupported_error(message.trim_end_matches('.'))
        }
        "invalid_request" => match error.message.split_once(": ") {
            // "<DOMException name>: message" from the node model is the DOMException the DOM
            // standard names, so `assert_throws_dom` and `e.name` see what Blink throws.
            Some((name, message)) if crate::v9::builtins::legacy_code(name) != 0 => {
                vm.dom_exception(name, message)
            }
            _ => Thrown::Fatal(error),
        },
        _ => Thrown::Fatal(error),
    }
}

/** The wrapper object for a node, created once and reused. */
pub(in crate::v9) fn wrap(vm: &mut Vm, node: usize) -> Res<JsValue> {
    if node == DOCUMENT {
        return Ok(JsValue::Object(realm().ids.document));
    }
    if let Some(id) = vm.page.state.wrappers.get(&node)
        && vm.heap.get(*id).is_some()
    {
        return Ok(JsValue::Object(*id));
    }
    if !vm.page.exists(node) {
        return Ok(JsValue::Null);
    }
    let tag = vm.page.tag(node);
    if tag == super::frames::DOCUMENT_NODE {
        return super::frames::wrap_document(vm, node);
    }
    let ids = &realm().ids;
    let proto = match tag.as_str() {
        "#text" | "#comment" => ids.text_prototype,
        PROCESSING_INSTRUCTION => {
            implementation::processing_instruction_prototype().unwrap_or(ids.text_prototype)
        }
        super::frames::DOCTYPE => super::frames::doctype_prototype().unwrap_or(ids.node_prototype),
        components::FRAGMENT => components::ids().fragment,
        components::SHADOW_ROOT => components::ids().shadow_root,
        _ => element_prototype_for(vm, node, &tag),
    };
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Node(node))),
        Some(proto),
    ))?;
    vm.page.state.wrappers.insert(node, id);
    Ok(JsValue::Object(id))
}

/** The prototype of an element wrapper: an element in the null namespace (made by an XML document
or createElementNS(null, ...)) is a plain Element, whatever its name; any other takes its interface
from its tag, as HTML and SVG do. */
fn element_prototype_for(vm: &Vm, node: usize, tag: &str) -> u32 {
    let ids = &realm().ids;
    let null_namespace = vm
        .page
        .state
        .element_ns
        .get(&node)
        .is_some_and(|record| record.namespace.is_none());
    if null_namespace {
        return ids.element_prototype;
    }
    super::interfaces::prototype_for(tag).unwrap_or(ids.html_element_prototype)
}

/** An element in the null namespace is a plain Element. Its wrapper was made before the namespace
was recorded, so it is relinked here; a custom element keeps its class, which the wrapper got first. */
fn relink_null_namespace(vm: &mut Vm, node: usize) -> Res<()> {
    let null_namespace = vm
        .page
        .state
        .element_ns
        .get(&node)
        .is_some_and(|record| record.namespace.is_none());
    let wrapper = vm.page.state.wrappers.get(&node).copied();
    if let (true, Some(id)) = (null_namespace, wrapper) {
        vm.set_proto(id, Some(realm().ids.element_prototype))?;
    }
    Ok(())
}

/** The node a value wraps. */
pub(in crate::v9) fn node_of(vm: &Vm, value: &JsValue) -> Option<usize> {
    if *value == JsValue::Object(realm().ids.document) {
        return Some(DOCUMENT);
    }
    match super::host_data(vm, value)? {
        HostData::Node(node) => Some(node),
        HostData::Document(html) => Some(html),
        _ => None,
    }
}

/** `this` as a node. */
pub(super) fn this_node(vm: &mut Vm, call: &Call) -> Res<usize> {
    node_of(vm, &call.this).ok_or_else(|| vm.type_error("Illegal invocation"))
}

/** An argument as a node. */
pub(super) fn arg_node(vm: &mut Vm, call: &Call, index: usize) -> Res<usize> {
    let value = call.arg(index);
    if let Some(node) = node_of(vm, &value) {
        return Ok(node);
    }
    if matches!(super::host_data(vm, &value), Some(HostData::Attr(_))) {
        return Err(vm.dom_exception(
            "HierarchyRequestError",
            "An attribute cannot be a child node.",
        ));
    }
    Err(vm.type_error("parameter is not of type 'Node'"))
}

/** Rewrite node identities after the document adopted script-created nodes. */
pub(in crate::v9) fn adopt(vm: &mut Vm, mapping: &[(usize, usize)]) -> Res<()> {
    for (old, new) in mapping {
        super::images::adopt(&mut vm.page, *old, *new)?;
        if let Some(wrapper) = vm.page.state.wrappers.remove(old) {
            if let Some(Class::Host(data)) =
                vm.heap.get_mut(wrapper).map(|object| &mut object.class)
            {
                **data = HostData::Node(*new);
            }
            vm.page.state.wrappers.insert(*new, wrapper);
        }
        components::adopt(&mut vm.page, *old, *new);
        super::scripts::adopt(&mut vm.page, *old, *new);
        super::frames::adopt(&mut vm.page, *old, *new);
        if vm.page.state.focused == Some(*old) {
            vm.page.state.focused = Some(*new);
        }
        for listener in &mut vm.page.state.listeners {
            if listener.target == super::Target::Node(*old) {
                listener.target = super::Target::Node(*new);
            }
        }
    }
    Ok(())
}

/** A static node list object. */
pub(in crate::v9) fn node_list(vm: &mut Vm, nodes: Vec<usize>) -> Res<JsValue> {
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::NodeList(nodes.into()))),
        Some(realm().ids.node_list_prototype),
    ))?;
    Ok(JsValue::Object(id))
}

/** A live list of a node's children (or element children). */
fn live_children(vm: &mut Vm, node: usize, elements: bool) -> Res<JsValue> {
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Children(node, elements))),
        Some(realm().ids.node_list_prototype),
    ))?;
    Ok(JsValue::Object(id))
}

/** The nodes a list object currently holds. */
fn list_nodes(vm: &mut Vm, data: &HostData) -> Vec<usize> {
    match data {
        HostData::NodeList(nodes) => nodes.to_vec(),
        HostData::Children(node, elements) => {
            let children = vm.page.children(*node);
            if *elements {
                children
                    .into_iter()
                    .filter(|child| vm.page.is_element(*child))
                    .collect()
            } else {
                children
            }
        }
        _ => Vec::new(),
    }
}

/** How many nodes a list object holds. A live children list counts from the tree's cache instead
of building the list, so `length` on a parent with many children costs O(1) after the first read. */
fn list_count(vm: &mut Vm, data: &HostData) -> usize {
    match data {
        HostData::NodeList(nodes) => nodes.len(),
        HostData::Children(node, elements) => vm.page.child_count(*node, *elements),
        _ => 0,
    }
}

/** NodeList.item(index): the node at that index, or null. Reads one node, not a copy of the list. */
fn list_item(vm: &mut Vm, call: &Call, data: &HostData) -> Res<JsValue> {
    let index = vm.to_integer(&call.arg(0))?;
    let found = match data {
        _ if index < 0.0 => None,
        HostData::NodeList(nodes) => nodes.get(index as usize).copied(),
        HostData::Children(node, elements) => vm.page.child_at(*node, *elements, index as usize),
        _ => None,
    };
    wrap_option(vm, found)
}

/** A link's href with its query written in the document's own encoding, so a page in Big5 reads back
the query bytes it wrote. UTF-8, an unknown label, or no label at all leaves the href as it is. */
fn href_in_document_encoding(vm: &Vm, href: &str) -> String {
    match charset::for_label(&vm.page.state.charset) {
        Some(encoding) if encoding.name() != UTF_8 => encode_url_query(href, encoding),
        _ => href.to_string(),
    }
}

/** A static NodeList's own index or `length`. The values come from the list's own vector, read
in place: copying the list for every read made each index read cost the list's length, so a loop
over a static list was quadratic in its size. `found` is the index lookup; it is ignored unless
`indexed` says the key was an index. */
fn static_list_own(
    vm: &mut Vm,
    indexed: bool,
    found: Option<usize>,
    length: usize,
    is_length: bool,
) -> Res<Option<Prop>> {
    let value = if indexed {
        match found {
            Some(node) => wrap(vm, node)?,
            None => return Ok(None),
        }
    } else if is_length {
        JsValue::Number(length as f64)
    } else {
        return Ok(None);
    };
    Ok(Some(Prop {
        slot: Slot::Data(value),
        attrs: crate::v9::vm::heap::ENUMERABLE,
    }))
}

/** A read-only, enumerable own data property, which list indexes, length and host fields are. */
fn data_prop(value: JsValue) -> Prop {
    Prop {
        slot: Slot::Data(value),
        attrs: crate::v9::vm::heap::ENUMERABLE,
    }
}

/**
 * A static list answers its index and length from the vector it holds. Copying that vector per
 * read made `list[i]` and `list.length` cost O(n) each, so a loop over a list cost O(n^2).
 */

/** Own virtual properties of host objects. */
pub(super) fn host_own(vm: &mut Vm, id: ObjectId, key: &Key) -> Res<Option<Prop>> {
    let Some(Class::Host(data)) = vm.heap.get(id).map(|object| &object.class) else {
        return Ok(None);
    };
    let Key::Str(name) = key else {
        return Ok(None);
    };
    if let HostData::NodeList(nodes) = &**data {
        let index = key.index();
        let found = index.and_then(|index| nodes.get(index as usize).copied());
        let length = nodes.len();
        return static_list_own(vm, index.is_some(), found, length, name.is("length"));
    }
    let data = (**data).clone();
    let readonly = |value: JsValue| Prop {
        slot: Slot::Data(value),
        attrs: crate::v9::vm::heap::ENUMERABLE,
    };
    match &data {
        HostData::Children(parent, elements) => {
            if let Some(index) = key.index() {
                return match vm.page.child_at(*parent, *elements, index as usize) {
                    Some(node) => Ok(Some(readonly(wrap(vm, node)?))),
                    None => Ok(None),
                };
            }
            if name.is("length") {
                let count = vm.page.child_count(*parent, *elements);
                return Ok(Some(readonly(JsValue::Number(count as f64))));
            }
            Ok(None)
        }
        HostData::Style(node) => {
            let text = name.lossy();
            if !style_property(&text) {
                return Ok(None);
            }
            let declarations = style_map(vm, *node);
            let value = declarations
                .iter()
                .find(|(property, _)| *property == css_name(&text))
                .map(|(_, value)| value.clone())
                .unwrap_or_default();
            Ok(Some(Prop::data(JsValue::string(&value)?)))
        }
        HostData::Computed(node) => {
            let text = name.lossy();
            if !style_property(&text) {
                return Ok(None);
            }
            let value = computed_value(vm, *node, &css_name(&text));
            Ok(Some(data_prop(JsValue::string(&value)?)))
        }
        HostData::Dataset(node) => {
            let attribute = format!("data-{}", kebab(&name.lossy()));
            let value = vm
                .page
                .view(*node)
                .and_then(|view| view.attributes.get(&attribute).cloned());
            match value {
                Some(value) => Ok(Some(Prop::data(JsValue::string(&value)?))),
                None => Ok(None),
            }
        }
        HostData::Storage(local) => {
            let text = name.lossy();
            if storage_method(&text) {
                return Ok(None);
            }
            let area = if *local {
                &vm.page.storage.local
            } else {
                &vm.page.storage.session
            };
            match area.get(&text) {
                Some(value) => Ok(Some(Prop::data(JsValue::string(value)?))),
                None => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

/** Host objects' own writes. */
pub(super) fn host_write(
    vm: &mut Vm,
    id: ObjectId,
    key: &Key,
    value: &JsValue,
) -> Res<Option<bool>> {
    let Some(Class::Host(data)) = vm.heap.get(id).map(|object| &object.class) else {
        return Ok(None);
    };
    let data = (**data).clone();
    let Key::Str(name) = key else {
        return Ok(None);
    };
    match data {
        HostData::Style(node) => {
            let property = name.lossy();
            if !style_property(&property) {
                return Ok(None);
            }
            let text = vm.to_text(value)?;
            set_style(vm, node, &css_name(&property), &text)?;
            Ok(Some(true))
        }
        HostData::Dataset(node) => {
            let text = vm.to_text(value)?;
            let attribute = format!("data-{}", kebab(&name.lossy()));
            vm.page
                .set_attribute(node, &attribute, Some(&text))
                .map_err(|error| dom_error(vm, error))?;
            components::flush(vm)?;
            Ok(Some(true))
        }
        HostData::Storage(local) => {
            let property = name.lossy();
            if storage_method(&property) {
                return Ok(None);
            }
            let text = vm.to_text(value)?;
            super::window::storage_set(vm, local, property, text)?;
            Ok(Some(true))
        }
        HostData::NodeList(_) | HostData::Children(..)
            if key.index().is_some() || name.is("length") =>
        {
            Ok(Some(false))
        }
        _ => Ok(None),
    }
}

/** Host objects' own deletes. */
pub(super) fn host_delete(vm: &mut Vm, id: ObjectId, key: &Key) -> Res<bool> {
    let Some(Class::Host(data)) = vm.heap.get(id).map(|object| &object.class) else {
        return Ok(false);
    };
    let data = (**data).clone();
    let Key::Str(name) = key else {
        return Ok(false);
    };
    match data {
        HostData::Dataset(node) => {
            let attribute = format!("data-{}", kebab(&name.lossy()));
            vm.page
                .set_attribute(node, &attribute, None)
                .map_err(|error| dom_error(vm, error))?;
            components::flush(vm)?;
            Ok(true)
        }
        HostData::Storage(local) => {
            super::window::storage_remove(vm, local, &name.lossy())?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/** Host objects' own virtual keys. */
pub(super) fn host_keys(vm: &mut Vm, id: ObjectId) -> Res<Vec<Key>> {
    let Some(Class::Host(data)) = vm.heap.get(id).map(|object| &object.class) else {
        return Ok(Vec::new());
    };
    let data = (**data).clone();
    let keys: Vec<String> = match &data {
        HostData::NodeList(_) | HostData::Children(..) => {
            let count = list_count(vm, &data);
            (0..count).map(|index| index.to_string()).collect()
        }
        HostData::Dataset(node) => vm
            .page
            .view(*node)
            .map(|view| {
                view.attributes
                    .keys()
                    .filter_map(|name| name.strip_prefix("data-"))
                    .map(camel)
                    .collect()
            })
            .unwrap_or_default(),
        HostData::Storage(local) => {
            let area = if *local {
                &vm.page.storage.local
            } else {
                &vm.page.storage.session
            };
            area.keys().cloned().collect()
        }
        _ => Vec::new(),
    };
    keys.iter()
        .map(|key| Ok(Key::Str(JsString::from_utf8(key)?)))
        .collect()
}

/** Storage methods are prototype members, not keys. */
fn storage_method(name: &str) -> bool {
    matches!(
        name,
        "getItem" | "setItem" | "removeItem" | "clear" | "key" | "length"
    )
}

/** Whether a style object key names a CSS property Chrome supports (so `in` checks match). */
fn style_property(name: &str) -> bool {
    let name = name
        .strip_prefix("webkit")
        .or_else(|| name.strip_prefix("Webkit"))
        .map_or(name.to_string(), |rest| {
            let mut chars = rest.chars();
            chars.next().map_or(String::new(), |first| {
                first.to_ascii_lowercase().to_string() + chars.as_str()
            })
        });
    let name = if name.contains('-') {
        camel(&name)
    } else {
        name
    };
    CSS_PROPERTIES.binary_search(&name.as_str()).is_ok()
}

/** CSS properties a style declaration exposes, camelCased and sorted for binary search. */
const CSS_PROPERTIES: [&str; 240] = [
    "accentColor",
    "alignContent",
    "alignItems",
    "alignSelf",
    "all",
    "animation",
    "animationDelay",
    "animationDirection",
    "animationDuration",
    "animationFillMode",
    "animationIterationCount",
    "animationName",
    "animationPlayState",
    "animationTimingFunction",
    "appearance",
    "aspectRatio",
    "backdropFilter",
    "backfaceVisibility",
    "background",
    "backgroundAttachment",
    "backgroundBlendMode",
    "backgroundClip",
    "backgroundColor",
    "backgroundImage",
    "backgroundOrigin",
    "backgroundPosition",
    "backgroundPositionX",
    "backgroundPositionY",
    "backgroundRepeat",
    "backgroundSize",
    "blockSize",
    "border",
    "borderBottom",
    "borderBottomColor",
    "borderBottomLeftRadius",
    "borderBottomRightRadius",
    "borderBottomStyle",
    "borderBottomWidth",
    "borderCollapse",
    "borderColor",
    "borderImage",
    "borderLeft",
    "borderLeftColor",
    "borderLeftStyle",
    "borderLeftWidth",
    "borderRadius",
    "borderRight",
    "borderRightColor",
    "borderRightStyle",
    "borderRightWidth",
    "borderSpacing",
    "borderStyle",
    "borderTop",
    "borderTopColor",
    "borderTopLeftRadius",
    "borderTopRightRadius",
    "borderTopStyle",
    "borderTopWidth",
    "borderWidth",
    "bottom",
    "boxDecorationBreak",
    "boxOrient",
    "boxShadow",
    "boxSizing",
    "breakAfter",
    "breakBefore",
    "breakInside",
    "captionSide",
    "caretColor",
    "clear",
    "clip",
    "clipPath",
    "color",
    "columnCount",
    "columnGap",
    "columnRule",
    "columnWidth",
    "columns",
    "contain",
    "content",
    "counterIncrement",
    "counterReset",
    "cssFloat",
    "cursor",
    "direction",
    "display",
    "emptyCells",
    "fill",
    "filter",
    "flex",
    "flexBasis",
    "flexDirection",
    "flexFlow",
    "flexGrow",
    "flexShrink",
    "flexWrap",
    "float",
    "font",
    "fontFamily",
    "fontFeatureSettings",
    "fontKerning",
    "fontOpticalSizing",
    "fontSize",
    "fontStretch",
    "fontStyle",
    "fontVariant",
    "fontVariationSettings",
    "fontWeight",
    "gap",
    "grid",
    "gridArea",
    "gridAutoColumns",
    "gridAutoFlow",
    "gridAutoRows",
    "gridColumn",
    "gridColumnEnd",
    "gridColumnStart",
    "gridRow",
    "gridRowEnd",
    "gridRowStart",
    "gridTemplate",
    "gridTemplateAreas",
    "gridTemplateColumns",
    "gridTemplateRows",
    "height",
    "hyphens",
    "inlineSize",
    "inset",
    "insetBlock",
    "insetInline",
    "isolation",
    "justifyContent",
    "justifyItems",
    "justifySelf",
    "left",
    "letterSpacing",
    "lineClamp",
    "lineHeight",
    "listStyle",
    "listStyleImage",
    "listStylePosition",
    "listStyleType",
    "margin",
    "marginBlock",
    "marginBottom",
    "marginInline",
    "marginLeft",
    "marginRight",
    "marginTop",
    "mask",
    "maxHeight",
    "maxInlineSize",
    "maxWidth",
    "minBlockSize",
    "minHeight",
    "minWidth",
    "mixBlendMode",
    "objectFit",
    "objectPosition",
    "opacity",
    "order",
    "outline",
    "outlineColor",
    "outlineOffset",
    "outlineStyle",
    "outlineWidth",
    "overflow",
    "overflowWrap",
    "overflowX",
    "overflowY",
    "overscrollBehavior",
    "padding",
    "paddingBlock",
    "paddingBottom",
    "paddingInline",
    "paddingLeft",
    "paddingRight",
    "paddingTop",
    "perspective",
    "placeContent",
    "placeItems",
    "placeSelf",
    "pointerEvents",
    "position",
    "quotes",
    "resize",
    "right",
    "rotate",
    "rowGap",
    "scale",
    "scrollBehavior",
    "scrollSnapType",
    "scrollbarColor",
    "scrollbarWidth",
    "stroke",
    "strokeWidth",
    "tabSize",
    "tableLayout",
    "textAlign",
    "textBox",
    "textBoxEdge",
    "textBoxTrim",
    "textDecoration",
    "textDecorationColor",
    "textDecorationLine",
    "textDecorationSkipInk",
    "textDecorationStyle",
    "textDecorationThickness",
    "textIndent",
    "textOverflow",
    "textShadow",
    "textSizeAdjust",
    "textTransform",
    "textUnderlineOffset",
    "textUnderlinePosition",
    "textWrap",
    "top",
    "touchAction",
    "transform",
    "transformOrigin",
    "transformStyle",
    "transition",
    "transitionDelay",
    "transitionDuration",
    "transitionProperty",
    "transitionTimingFunction",
    "translate",
    "unicodeBidi",
    "userSelect",
    "verticalAlign",
    "visibility",
    "whiteSpace",
    "width",
    "willChange",
    "wordBreak",
    "wordSpacing",
    "wordWrap",
    "writingMode",
    "zIndex",
    "zoom",
];

/** A style attribute's CSS property: kebab-case, with CSSOM's vendor prefix rule, so
`webkitLineClamp` and `WebkitLineClamp` both name `-webkit-line-clamp`. */
fn css_name(attribute: &str) -> String {
    let prefixed = ["webkit", "Webkit"].iter().find_map(|prefix| {
        attribute
            .strip_prefix(prefix)
            .filter(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase()))
    });
    match prefixed {
        Some(rest) => format!("-webkit{}", kebab(rest)),
        None => kebab(attribute),
    }
}

/** camelCase to kebab-case (`backgroundColor` to `background-color`, `cssFloat` to `float`). */
pub(in crate::v9) fn kebab(name: &str) -> String {
    if name == "cssFloat" {
        return "float".into();
    }
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/** kebab-case to camelCase. */
pub(in crate::v9) fn camel(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in name.chars() {
        if c == '-' {
            upper = true;
        } else if upper {
            out.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/** The inline style declarations of a node. */
pub(in crate::v9) fn style_map(vm: &mut Vm, node: usize) -> Vec<(String, String)> {
    let text = vm
        .page
        .view(node)
        .and_then(|view| view.attributes.get("style").cloned())
        .unwrap_or_default();
    text.split(';')
        .filter_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .filter(|(name, _)| !name.is_empty())
        .collect()
}

/** Elements that display as blocks by default. */
const BLOCK_TAGS: [&str; 31] = [
    "address",
    "article",
    "aside",
    "blockquote",
    "body",
    "dd",
    "details",
    "dialog",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "html",
    "main",
    "nav",
    "ol",
    "p",
    "section",
    "ul",
];

/** A computed value: the inline declaration, else the last style pass's `position`, else the CSS
initial or UA default. */
pub(in crate::v9) fn computed_value(vm: &mut Vm, node: usize, property: &str) -> String {
    if property == "opacity"
        && let Some(value) = super::animations::opacity(vm, node)
    {
        return value.to_string();
    }
    if let Some((_, value)) = style_map(vm, node)
        .into_iter()
        .find(|(name, _)| name == property)
    {
        return value;
    }
    let tag = vm.page.tag(node);
    let hidden = vm
        .page
        .view(node)
        .is_some_and(|view| view.attributes.contains_key("hidden"));
    match property {
        "display"
            if hidden
                || matches!(
                    tag.as_str(),
                    "head" | "script" | "style" | "template" | "meta" | "link" | "title"
                ) =>
        {
            "none"
        }
        "display" if BLOCK_TAGS.contains(&tag.as_str()) => "block",
        "display" if tag == "li" => "list-item",
        "display" if matches!(tag.as_str(), "table") => "table",
        "display" if matches!(tag.as_str(), "tr") => "table-row",
        "display" if matches!(tag.as_str(), "td" | "th") => "table-cell",
        "display"
            if matches!(
                tag.as_str(),
                "button" | "input" | "select" | "textarea" | "img"
            ) =>
        {
            "inline-block"
        }
        "display" => "inline",
        "visibility" => "visible",
        "position" => {
            return vm
                .page
                .environment
                .positions
                .get(&node)
                .map_or_else(|| "static".to_owned(), Clone::clone);
        }
        "opacity" => "1",
        "float" => "none",
        "overflow" | "overflow-x" | "overflow-y" => "visible",
        "box-sizing" => "content-box",
        "direction" => "ltr",
        "z-index" | "width" | "height" | "top" | "left" | "right" | "bottom" => "auto",
        "font-size" => "16px",
        "color" => "rgb(0, 0, 0)",
        "background-color" => "rgba(0, 0, 0, 0)",
        _ if property.starts_with("margin")
            || property.starts_with("padding")
            || property.ends_with("width") =>
        {
            "0px"
        }
        _ => "",
    }
    .to_string()
}

/** Whether a connected element would render: no display:none or hidden on it or an ancestor. */
pub(in crate::v9) fn rendered(vm: &mut Vm, node: usize) -> bool {
    if !vm.page.connected(node) {
        return false;
    }
    let mut chain = vec![node];
    chain.extend(vm.page.ancestors(node));
    chain
        .into_iter()
        .all(|id| computed_value(vm, id, "display") != "none")
}

/** Set (or with an empty value remove) one inline style property. */
pub(in crate::v9) fn set_style(vm: &mut Vm, node: usize, property: &str, value: &str) -> Res<()> {
    let mut declarations = style_map(vm, node);
    declarations.retain(|(name, _)| name != property);
    if !value.trim().is_empty() {
        declarations.push((property.to_string(), value.trim().to_string()));
    }
    let text: Vec<String> = declarations
        .iter()
        .map(|(name, value)| format!("{name}: {value};"))
        .collect();
    let text = text.join(" ");
    let result = if text.is_empty() {
        vm.page.set_attribute(node, "style", None)
    } else {
        vm.page.set_attribute(node, "style", Some(&text))
    };
    result.map_err(|error| dom_error(vm, error))?;
    components::flush(vm)
}

/** The name an accessor native was registered under. */
pub(in crate::v9) fn native_name(call: &Call) -> &'static str {
    realm().native(call.callee).map_or("", |def| def.name)
}

/** The element a bare global name reaches through the window's named properties (HTML's
named access on the Window object): the first element in tree order whose id is the name.
Only a name no binding or property answers gets here, so scripts writing `dut.foo` for
`<div id=dut>` work as in Blink's WindowProperties. */
pub(in crate::v9) fn named_element(vm: &mut Vm, name: &str) -> Res<Option<JsValue>> {
    if name.is_empty() || vm.page.legacy || vm.page.document.is_none() {
        return Ok(None);
    }
    let found = search(vm, DOCUMENT, |page, id| {
        page.view(id)
            .is_some_and(|view| view.attributes.get("id").is_some_and(|value| value == name))
    });
    match found.first() {
        Some(node) => Ok(Some(wrap(vm, *node)?)),
        None => Ok(None),
    }
}

/** Wrap an optional node. */
pub(super) fn wrap_option(vm: &mut Vm, node: Option<usize>) -> Res<JsValue> {
    match node {
        Some(node) => wrap(vm, node),
        None => Ok(JsValue::Null),
    }
}

/** Children of a traversal node, including the document. */
fn traverse_children(vm: &mut Vm, node: usize) -> Vec<usize> {
    if node == DOCUMENT {
        return vec![HTML];
    }
    vm.page.children(node)
}

/** Parent for traversal: html's parent is the document. */
fn traverse_parent(vm: &mut Vm, node: usize) -> Option<usize> {
    match node {
        DOCUMENT => None,
        HTML => Some(DOCUMENT),
        _ if vm.page.has_shadows() && vm.page.is_shadow_root(node) => None,
        _ => vm.page.parent(node),
    }
}

/** Siblings by offset. */
fn sibling(vm: &mut Vm, node: usize, offset: isize, elements: bool) -> Option<usize> {
    let parent = traverse_parent(vm, node)?;
    let mut children = traverse_children(vm, parent);
    if elements {
        children.retain(|child| *child == node || vm.page.is_element(*child));
    }
    let position = children.iter().position(|child| *child == node)? as isize;
    children
        .get(usize::try_from(position + offset).ok()?)
        .copied()
}

/** A node's ownerDocument: the document it sits in, else the document it was made for (a
doctype or processing instruction from a script-made document), else the page's. The page's own
document and documents have none. */
fn owner_document(vm: &mut Vm, node: usize) -> Res<JsValue> {
    if node == DOCUMENT || vm.page.tag(node) == super::frames::DOCUMENT_NODE {
        return Ok(JsValue::Null);
    }
    let owner = owning_document_id(vm, node);
    wrap(vm, owner)
}

/** The document a node belongs to: its tree's document node, else the document recorded for a
detached root, else the page document. Each step is a map lookup or the ancestor walk. */
fn owning_document_id(vm: &mut Vm, node: usize) -> usize {
    let root = vm.page.ancestors(node).last().copied().unwrap_or(node);
    if vm.page.tag(root) == super::frames::DOCUMENT_NODE {
        return root;
    }
    vm.page.state.owners.get(&root).copied().unwrap_or(DOCUMENT)
}

/** An attribute a node keeps for its own sake (a doctype's name, a processing instruction's
target), or the empty string when it has none. */
fn stored_attribute(vm: &mut Vm, node: usize, name: &str) -> String {
    vm.page
        .view(node)
        .and_then(|view| view.attributes.get(name).cloned())
        .unwrap_or_default()
}

/** Node accessors and methods, dispatched by registered name. */
pub(in crate::v9) fn node_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = this_node(vm, &call)?;
    let name = native_name(&call);
    match name {
        "nodeType" => Ok(JsValue::Number(f64::from(node_type(vm, node)))),
        "nodeName" => {
            let tag = if node == DOCUMENT {
                "#document".to_string()
            } else {
                vm.page.tag(node)
            };
            let name = match tag.as_str() {
                super::frames::DOCTYPE => stored_attribute(vm, node, "name"),
                PROCESSING_INSTRUCTION => stored_attribute(vm, node, "target"),
                _ if tag.starts_with('#') => tag.clone(),
                _ => qualified_name(vm, node),
            };
            JsValue::string(&name).map_err(Into::into)
        }
        "parentNode" => {
            let parent = traverse_parent(vm, node);
            wrap_option(vm, parent)
        }
        "parentElement" => {
            let parent = traverse_parent(vm, node).filter(|parent| *parent != DOCUMENT);
            wrap_option(vm, parent)
        }
        "childNodes" => live_children(vm, node, false),
        "firstChild" => {
            let first = traverse_children(vm, node).first().copied();
            wrap_option(vm, first)
        }
        "lastChild" => {
            let last = traverse_children(vm, node).last().copied();
            wrap_option(vm, last)
        }
        "nextSibling" => {
            let next = sibling(vm, node, 1, false);
            wrap_option(vm, next)
        }
        "previousSibling" => {
            let previous = sibling(vm, node, -1, false);
            wrap_option(vm, previous)
        }
        "ownerDocument" => owner_document(vm, node),
        "isConnected" => Ok(JsValue::Boolean(
            node == DOCUMENT || vm.page.connected(node),
        )),
        "textContent" | "innerText" | "nodeValue" | "data" => {
            let tag = vm.page.tag(node);
            if node == DOCUMENT
                || tag == super::frames::DOCUMENT_NODE
                || tag == super::frames::DOCTYPE
            {
                return Ok(JsValue::Null);
            }
            if name == "nodeValue" && !tag.starts_with('#') {
                return Ok(JsValue::Null);
            }
            let text = vm.page.text_content(node);
            JsValue::string(&text).map_err(Into::into)
        }
        "length" => {
            let text = vm.page.text_content(node);
            Ok(JsValue::Number(text.encode_utf16().count() as f64))
        }
        _ => Ok(JsValue::Undefined),
    }
}

/** Node setters (textContent and friends). */
/** Runs the steps below, then the custom element reactions and records they queued. */
pub(in crate::v9) fn node_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| node_set_steps(vm, call))
}

/** The node_set steps themselves. */
fn node_set_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = this_node(vm, &call)?;
    let value = call.arg(0);
    let text = if value.nullish() {
        String::new()
    } else {
        vm.to_text(&value)?
    };
    if node != DOCUMENT {
        let tag = vm.page.tag(node);
        if tag.starts_with('#') && tag != components::FRAGMENT && tag != components::SHADOW_ROOT {
            set_data(vm, node, &text)?;
        } else {
            let removed = vm.page.children(node);
            clear_children_hooks(vm, node)?;
            vm.page
                .set_text(node, &text)
                .map_err(|error| dom_error(vm, error))?;
            let added = vm.page.children(node);
            children_changed(vm, node, added, removed);
        }
    }
    Ok(JsValue::Undefined)
}

/** Replace a character data node's whole text (the data setter's replace-data over all of it). */
fn set_data(vm: &mut Vm, node: usize, text: &str) -> Res<()> {
    let length = vm.page.text_content(node).encode_utf16().count();
    replace_data(vm, node, 0, length, text)
}

/**
 * Replace `count` UTF-16 units of a character data node at `offset` with `data`, queueing the
 * characterData record and moving the live range points inside the span as the standard does.
 * Callers keep offset and count within the node's length.
 */
pub(super) fn replace_data(
    vm: &mut Vm,
    node: usize,
    offset: usize,
    count: usize,
    data: &str,
) -> Res<()> {
    let units: Vec<u16> = vm.page.text_content(node).encode_utf16().collect();
    let tail = (offset + count).min(units.len());
    let mut next = units[..offset.min(units.len())].to_vec();
    next.extend(data.encode_utf16());
    next.extend_from_slice(&units[tail..]);
    let old = vm.page.text_content(node);
    vm.page
        .set_text(node, &String::from_utf16_lossy(&next))
        .map_err(|error| dom_error(vm, error))?;
    vm.page
        .mutations
        .push(components::Mutation::Text { node, old });
    let inserted = data.encode_utf16().count();
    range_live::replaced(vm, node, offset, count, inserted)
}

/** A WebIDL `unsigned long` argument: ToNumber, then modulo 2^32. */
fn unsigned_long(vm: &mut Vm, value: &JsValue) -> Res<usize> {
    let number = vm.to_number(value)?;
    if !number.is_finite() {
        return Ok(0);
    }
    Ok(number.trunc().rem_euclid(4_294_967_296.0) as usize)
}

/** CharacterData's substringData, appendData, insertData, deleteData and replaceData, and
Text's splitText, in UTF-16 code units as the DOM standard counts them (Blink's
CharacterData::replaceData). An offset past the length is IndexSizeError; a count runs at
most to the end. */
pub(super) fn character_data_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = this_node(vm, &call)?;
    let name = native_name(&call);
    if matches!(name, "before" | "after" | "replaceWith" | "remove") {
        if name == "remove" {
            if traverse_parent(vm, node).is_some() {
                remove_node(vm, node)?;
            }
            return Ok(JsValue::Undefined);
        }
        return components::reacting(vm, |vm| mutate(vm, node, &call));
    }
    let units: Vec<u16> = vm.page.text_content(node).encode_utf16().collect();
    let length = units.len();
    let required = match name {
        "appendData" => 1,
        "substringData" | "insertData" | "deleteData" => 2,
        "replaceData" => 3,
        _ => 1,
    };
    if call.args.len() < required {
        return Err(vm.type_error(&format!("{required} arguments required.")));
    }
    if name == "appendData" {
        let data = vm.to_text(&call.arg(0))?;
        replace_data(vm, node, length, 0, &data)?;
        return Ok(JsValue::Undefined);
    }
    let offset = unsigned_long(vm, &call.arg(0))?;
    if offset > length {
        return Err(vm.dom_exception(
            "IndexSizeError",
            "The offset is larger than the node's length.",
        ));
    }
    let count = match name {
        "insertData" | "splitText" => 0,
        _ => unsigned_long(vm, &call.arg(1))?.min(length - offset),
    };
    match name {
        "substringData" => {
            JsValue::string(&String::from_utf16_lossy(&units[offset..offset + count]))
                .map_err(Into::into)
        }
        "splitText" => {
            let created = split_text_node(vm, node, offset)?;
            wrap(vm, created)
        }
        _ => {
            let inserted = match name {
                "insertData" => vm.to_text(&call.arg(1))?,
                "replaceData" => vm.to_text(&call.arg(2))?,
                _ => String::new(),
            };
            replace_data(vm, node, offset, count, &inserted)?;
            Ok(JsValue::Undefined)
        }
    }
}

/**
 * Text.splitText as the DOM standard runs it: the data from `offset` moves into a new text node
 * placed after this one. The insertion moves the live points after it; the points past the split
 * then follow the data, and the data leaves this node through replaceData. Returns the new node.
 */
pub(super) fn split_text_node(vm: &mut Vm, node: usize, offset: usize) -> Res<usize> {
    let units: Vec<u16> = vm.page.text_content(node).encode_utf16().collect();
    let length = units.len();
    if offset > length {
        return Err(vm.dom_exception(
            "IndexSizeError",
            "Failed to execute 'splitText' on 'Text': The offset is larger than the Text node's length.",
        ));
    }
    let rest = String::from_utf16_lossy(&units[offset..]);
    let created = vm
        .page
        .create("#text", Some(rest))
        .map_err(|error| dom_error(vm, error))?;
    let parent = traverse_parent(vm, node);
    let index = parent.map_or(0, |parent| index_of(vm, parent, node));
    if let Some(parent) = parent {
        let next = sibling(vm, node, 1, false);
        insert(vm, parent, created, next)?;
    }
    range_live::split(vm, node, offset, created, parent, index)?;
    replace_data(vm, node, offset, length - offset, "")?;
    Ok(created)
}

/** The index of `node` among `parent`'s children, or 0 when it is not one of them. */
pub(super) fn index_of(vm: &mut Vm, parent: usize, node: usize) -> usize {
    vm.page
        .children(parent)
        .iter()
        .position(|child| *child == node)
        .unwrap_or(0)
}

/**
 * The live-range step of a replace-all: each child leaves at index 0 in turn, which moves the
 * points inside it to the parent's start and pulls points past the start back to it.
 */
pub(super) fn clear_children_hooks(vm: &mut Vm, parent: usize) -> Res<()> {
    for child in vm.page.children(parent) {
        range_live::removed(vm, parent, 0, child)?;
    }
    Ok(())
}

/** Queue a child list change of `parent` for reactions and observers. */
pub(super) fn children_changed(vm: &mut Vm, parent: usize, added: Vec<usize>, removed: Vec<usize>) {
    if added.is_empty() && removed.is_empty() {
        return;
    }
    vm.page.mutations.push(components::Mutation::Children {
        parent,
        added,
        removed,
        previous: None,
        next: None,
    });
}

/** Remove a node from its parent, queueing the change. */
pub(in crate::v9) fn remove_node(vm: &mut Vm, node: usize) -> Res<()> {
    let parent = vm.page.parent(node);
    let siblings = parent.map(|parent| vm.page.children(parent));
    if let Some(parent) = parent {
        let index = index_of(vm, parent, node);
        range_live::removed(vm, parent, index, node)?;
    }
    vm.page.remove(node).map_err(|error| dom_error(vm, error))?;
    if let (Some(parent), Some(siblings)) = (parent, siblings) {
        let position = siblings.iter().position(|child| *child == node);
        vm.page.mutations.push(components::Mutation::Children {
            parent,
            added: Vec::new(),
            removed: vec![node],
            previous: position
                .and_then(|index| index.checked_sub(1))
                .and_then(|index| siblings.get(index).copied()),
            next: position.and_then(|index| siblings.get(index + 1).copied()),
        });
    }
    Ok(())
}

/** Node methods. */
/** Runs the steps below, then the custom element reactions and records they queued. */
pub(in crate::v9) fn node_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| node_method_steps(vm, call))
}

/** The node_method steps themselves. */
fn node_method_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = this_node(vm, &call)?;
    match native_name(&call) {
        "appendChild" => {
            let child = arg_node(vm, &call, 0)?;
            ensure_pre_insert(vm, node, child, None, false)?;
            insert(vm, node, child, None)?;
            Ok(call.arg(0))
        }
        "insertBefore" => {
            let child = arg_node(vm, &call, 0)?;
            let reference = if call.arg(1).nullish() {
                None
            } else {
                Some(arg_node(vm, &call, 1)?)
            };
            ensure_pre_insert(vm, node, child, reference, false)?;
            insert(vm, node, child, reference)?;
            Ok(call.arg(0))
        }
        "removeChild" => {
            let child = arg_node(vm, &call, 0)?;
            if traverse_parent(vm, child) != Some(node) {
                return Err(vm.dom_exception(
                    "NotFoundError",
                    "The node to be removed is not a child of this node.",
                ));
            }
            remove_node(vm, child)?;
            Ok(call.arg(0))
        }
        "replaceChild" => {
            let new = arg_node(vm, &call, 0)?;
            let old = arg_node(vm, &call, 1)?;
            replace_child(vm, node, new, old)?;
            Ok(call.arg(1))
        }
        "hasChildNodes" => Ok(JsValue::Boolean(!traverse_children(vm, node).is_empty())),
        "contains" => {
            let Some(other) = node_of(vm, &call.arg(0)) else {
                return Ok(JsValue::Boolean(false));
            };
            if other == node || node == DOCUMENT && vm.page.connected(other) {
                return Ok(JsValue::Boolean(true));
            }
            Ok(JsValue::Boolean(vm.page.ancestors(other).contains(&node)))
        }
        "cloneNode" => {
            let deep = call.arg(0).truthy();
            let copy = clone_node(vm, node, deep)?;
            wrap(vm, copy)
        }
        "getRootNode" => {
            let composed = components::option(vm, &call.arg(0), "composed")?;
            if !composed && let Some(root) = components::containing_root(&mut vm.page, node) {
                return wrap(vm, root);
            }
            if vm.page.connected(node) {
                return Ok(JsValue::Object(realm().ids.document));
            }
            let ancestors = vm.page.ancestors(node);
            let root = ancestors.last().copied().unwrap_or(node);
            wrap(vm, root)
        }
        "compareDocumentPosition" => {
            let other = arg_node(vm, &call, 0)?;
            Ok(JsValue::Number(compare_position(vm, node, other)))
        }
        "isSameNode" => Ok(JsValue::Boolean(node_of(vm, &call.arg(0)) == Some(node))),
        "lookupNamespaceURI" => {
            let prefix = optional_string_arg(vm, &call, 0)?;
            optional_string(lookup_context(vm, node, prefix.as_deref()))
        }
        "isDefaultNamespace" => {
            let namespace = optional_string_arg(vm, &call, 0)?;
            Ok(JsValue::Boolean(
                namespace == lookup_context(vm, node, None),
            ))
        }
        "isEqualNode" => {
            let Some(other) = node_of(vm, &call.arg(0)) else {
                return Ok(JsValue::Boolean(false));
            };
            Ok(JsValue::Boolean(equal_nodes(vm, node, other)))
        }
        "normalize" => {
            normalize(vm, node)?;
            Ok(JsValue::Undefined)
        }
        _ => Ok(JsValue::Undefined),
    }
}

/** The namespace a lookup from a node finds for a prefix: an element searches itself, a document
searches its root, a doctype or fragment finds none, and any other node searches its parent. */
fn lookup_context(vm: &mut Vm, node: usize, prefix: Option<&str>) -> Option<String> {
    match node_type(vm, node) {
        1 => vm.page.lookup_namespace(node, prefix),
        9 => vm.page.lookup_namespace(HTML, prefix),
        10 | 11 => None,
        _ => {
            let parent = vm.page.parent(node)?;
            vm.page.lookup_namespace(parent, prefix)
        }
    }
}

/** createElement and createElementNS. createElement lowercases the name and puts it in the HTML
namespace; createElementNS validates and extracts the namespace and qualified name. Either way
the element's namespace record is kept, so its namespaceURI and tagName are the DOM's. */
/** How createElement treats its name and namespace on the document it is called on, per the DOM:
an HTML document lowercases the name and uses the HTML namespace, an XHTML document keeps the name
and uses the HTML namespace, and any other XML document keeps the name in the null namespace. */
fn create_element_rules(vm: &Vm, call: &Call) -> (bool, Option<&'static str>) {
    let media = match node_of(vm, &call.this) {
        Some(document) => super::frames::content_type_of(vm, document),
        None => super::frames::HTML_TYPE.to_string(),
    };
    match media.as_str() {
        super::frames::HTML_TYPE => (true, Some(HTML_NAMESPACE)),
        super::frames::XHTML_TYPE => (false, Some(HTML_NAMESPACE)),
        _ => (false, None),
    }
}

/** createElement and createElementNS. createElement lowercases the name and puts it in the HTML
namespace; createElementNS validates and extracts the namespace and qualified name. Either way
the element's namespace record is kept, so its namespaceURI and tagName are the DOM's. */
fn create_element_call(vm: &mut Vm, call: &Call, namespaced: bool) -> Res<JsValue> {
    let (qualified, record, options) = if namespaced {
        let namespace = optional_string_arg(vm, call, 0)?;
        let qualified = vm.to_text(&call.arg(1))?;
        let qname =
            namespaces::validate_and_extract(namespace.as_deref(), &qualified, NameKind::Element)
                .map_err(|error| name_error(vm, error))?;
        let record = ElementNs {
            namespace,
            prefix: qname.prefix,
            local: qname.local,
        };
        (qualified, record, call.arg(2))
    } else {
        let (lowercase, namespace) = create_element_rules(vm, call);
        let given = vm.to_text(&call.arg(0))?;
        let qualified = if lowercase {
            given.to_ascii_lowercase()
        } else {
            given
        };
        namespaces::validate_local_name(&qualified, NameKind::Element)
            .map_err(|error| name_error(vm, error))?;
        let record = ElementNs {
            namespace: namespace.map(str::to_string),
            prefix: None,
            local: qualified.clone(),
        };
        (qualified, record, call.arg(1))
    };
    let value = components::create_element(vm, &qualified, &options)?;
    if let Some(node) = node_of(vm, &value) {
        vm.page.set_element_namespace(node, record);
        relink_null_namespace(vm, node)?;
        if let Some(document) = node_of(vm, &call.this).filter(|document| *document != DOCUMENT) {
            vm.page.state.owners.insert(node, document);
        }
    }
    Ok(value)
}

/** The DOM standard's "equals": same type, same name and attributes (or data), and equal
children in order. */
fn equal_nodes(vm: &mut Vm, a: usize, b: usize) -> bool {
    if a == b {
        return true;
    }
    if node_type(vm, a) != node_type(vm, b) || vm.page.tag(a) != vm.page.tag(b) {
        return false;
    }
    if vm.page.element_namespace(a) != vm.page.element_namespace(b)
        || vm.page.element_prefix(a) != vm.page.element_prefix(b)
    {
        return false;
    }
    let (left, right) = (vm.page.view(a), vm.page.view(b));
    let same_attributes = match (&left, &right) {
        (Some(left), Some(right)) => left.attributes == right.attributes,
        (None, None) => true,
        _ => false,
    };
    if !same_attributes {
        return false;
    }
    if matches!(node_type(vm, a), 3 | 8) && vm.page.text_content(a) != vm.page.text_content(b) {
        return false;
    }
    let (left, right) = (traverse_children(vm, a), traverse_children(vm, b));
    left.len() == right.len()
        && left
            .iter()
            .zip(&right)
            .all(|(l, r)| equal_nodes(vm, *l, *r))
}

/** Node.normalize: drop empty text nodes and merge each run of adjacent text nodes into its
first, through the whole subtree. */
fn normalize(vm: &mut Vm, node: usize) -> Res<()> {
    let mut first: Option<usize> = None;
    for child in traverse_children(vm, node) {
        if node_type(vm, child) != 3 {
            first = None;
            normalize(vm, child)?;
            continue;
        }
        match first {
            Some(text) => merge_text(vm, text, child)?,
            None if vm.page.text_content(child).is_empty() => remove_node(vm, child)?,
            None => first = Some(child),
        }
    }
    Ok(())
}

/**
 * normalize folds the text node `child` into `first`, the text node before it: `child`'s data is
 * appended, the points in `child` move to `first` past the old text, and the node is removed.
 */
fn merge_text(vm: &mut Vm, first: usize, child: usize) -> Res<()> {
    let length = vm.page.text_content(first).encode_utf16().count();
    let parent = vm.page.parent(child);
    let index = parent.map_or(0, |parent| index_of(vm, parent, child));
    let data = vm.page.text_content(child);
    replace_data(vm, first, length, 0, &data)?;
    range_live::merged(vm, first, child, parent, index, length)?;
    remove_node(vm, child)
}

/** DOCUMENT_POSITION bits for compareDocumentPosition. */
pub(super) fn compare_position(vm: &mut Vm, node: usize, other: usize) -> f64 {
    if node == other {
        return 0.0;
    }
    let connected = |vm: &mut Vm, id: usize| id == DOCUMENT || vm.page.connected(id);
    if connected(vm, node) != connected(vm, other) {
        return 1.0 + 32.0 + 4.0;
    }
    if node == DOCUMENT || vm.page.ancestors(other).contains(&node) {
        return 16.0 + 4.0;
    }
    if other == DOCUMENT || vm.page.ancestors(node).contains(&other) {
        return 8.0 + 2.0;
    }
    let mut order = vec![HTML, HEAD];
    order.extend(vm.page.descendants(HEAD));
    order.push(BODY);
    order.extend(vm.page.descendants(BODY));
    let a = order.iter().position(|id| *id == node);
    let b = order.iter().position(|id| *id == other);
    match (a, b) {
        (Some(a), Some(b)) if a < b => 4.0,
        (Some(_), Some(_)) => 2.0,
        _ => 1.0 + 32.0 + 4.0,
    }
}

/** Move focus to a connected element: the old one gets `blur` and `focusout`, the new one
`focus` and `focusin`, as a click or a person's typing would give them. */
pub(in crate::v9) fn focus(vm: &mut Vm, node: usize) -> Res<()> {
    if vm.page.state.focused == Some(node) || !vm.page.connected(node) {
        return Ok(());
    }
    blur(vm)?;
    vm.page.state.focused = Some(node);
    tell_focus(vm, Some(node))?;
    super::events::fire(vm, super::Target::Node(node), "focus", false, false)?;
    super::events::fire(vm, super::Target::Node(node), "focusin", true, false)?;
    Ok(())
}

/** Tell the document engine where focus now is, for `:focus` and `:focus-within`. Only
projected elements have an identity there; focus elsewhere reads as none. */
fn tell_focus(vm: &mut Vm, node: Option<usize>) -> Res<()> {
    if vm.page.environment.dom_level != crate::v9::DomLevel::Extended {
        return Ok(());
    }
    let node = node.filter(|node| vm.page.is_projected(*node));
    vm.page
        .record(crate::document::ScriptChange::Focus { node })
        .map_err(|error| dom_error(vm, error))
}

/** Take focus away from the focused element, if any, firing `blur` and `focusout`. */
pub(in crate::v9) fn blur(vm: &mut Vm) -> Res<()> {
    let Some(previous) = vm.page.state.focused.take() else {
        return Ok(());
    };
    tell_focus(vm, None)?;
    if vm.page.exists(previous) {
        super::events::fire(vm, super::Target::Node(previous), "blur", false, false)?;
        super::events::fire(vm, super::Target::Node(previous), "focusout", true, false)?;
    }
    Ok(())
}

/** The DOM nodeType of a node (1 element, 3 text, 8 comment, 9 document, 10 doctype, 11
fragment; shadow roots are fragments). */
pub(super) fn node_type(vm: &mut Vm, node: usize) -> u8 {
    if node == DOCUMENT {
        return 9;
    }
    match vm.page.tag(node).as_str() {
        "#text" => 3,
        "#comment" => 8,
        PROCESSING_INSTRUCTION => 7,
        components::FRAGMENT | components::SHADOW_ROOT => 11,
        super::frames::DOCUMENT_NODE => 9,
        super::frames::DOCTYPE => 10,
        _ => 1,
    }
}

/** A thrown HierarchyRequestError. */
fn hierarchy(vm: &mut Vm, message: &str) -> Thrown {
    vm.dom_exception("HierarchyRequestError", message)
}

/** The DOM standard's "ensure pre-insertion validity" (and, with `replacing`, the checks of
"replace a child"): which parent, node and child combinations throw, and as what. Blink runs
the same checks in ContainerNode::EnsurePreInsertionValidity before any tree change, so a
rejected call leaves the tree untouched. */
pub(super) fn ensure_pre_insert(
    vm: &mut Vm,
    parent: usize,
    node: usize,
    child: Option<usize>,
    replacing: bool,
) -> Res<()> {
    let parent_type = node_type(vm, parent);
    if !matches!(parent_type, 1 | 9 | 11) {
        return Err(hierarchy(vm, "The parent cannot have children."));
    }
    if node == parent || vm.page.ancestors(parent).contains(&node) || node == DOCUMENT {
        return Err(hierarchy(vm, "The new child element contains the parent."));
    }
    if let Some(child) = child
        && traverse_parent(vm, child) != Some(parent)
    {
        let message = if replacing {
            "The node to be replaced is not a child of this node."
        } else {
            "The node before which the new node is to be inserted is not a child of this node."
        };
        return Err(vm.dom_exception("NotFoundError", message));
    }
    let node_kind = node_type(vm, node);
    if node_kind == 9 {
        return Err(hierarchy(vm, "A document cannot be inserted."));
    }
    if node_kind == 3 && parent_type == 9 || node_kind == 10 && parent_type != 9 {
        return Err(hierarchy(vm, "The node cannot be inserted here."));
    }
    if parent_type == 9 {
        document_child_rules(vm, parent, node, node_kind, child, replacing)?;
    }
    Ok(())
}

/** The document branch of pre-insertion validity: one element, one doctype, the doctype first. */
fn document_child_rules(
    vm: &mut Vm,
    parent: usize,
    node: usize,
    kind: u8,
    child: Option<usize>,
    replacing: bool,
) -> Res<()> {
    let all = traverse_children(vm, parent);
    let kinds: Vec<u8> = all.iter().map(|c| node_type(vm, *c)).collect();
    let at = child.and_then(|c| all.iter().position(|x| *x == c));
    // The document's children of one type, leaving out the child being replaced.
    let others = |wanted: u8| {
        kinds
            .iter()
            .enumerate()
            .filter(move |(i, k)| **k == wanted && !(replacing && Some(*i) == at))
            .map(|(i, _)| i)
    };
    let elements = match kind {
        11 => {
            let inner = vm.page.children(node);
            let inner: Vec<u8> = inner.iter().map(|c| node_type(vm, *c)).collect();
            if inner.contains(&3) {
                return Err(hierarchy(vm, "Text cannot be a child of a document."));
            }
            inner.iter().filter(|k| **k == 1).count()
        }
        1 => 1,
        10 => {
            let element_before = others(1).any(|i| at.is_none_or(|a| i < a));
            if others(10).next().is_some() || element_before {
                return Err(hierarchy(vm, "The document cannot take this doctype."));
            }
            return Ok(());
        }
        _ => return Ok(()),
    };
    let doctype_after = at.is_some_and(|a| others(10).any(|i| i > a));
    let child_is_doctype = !replacing && at.is_some_and(|a| kinds[a] == 10);
    let taken = others(1).next().is_some() || doctype_after || child_is_doctype;
    if elements > 1 || elements == 1 && taken {
        return Err(hierarchy(
            vm,
            "The document can only have one element child.",
        ));
    }
    Ok(())
}

/** The DOM standard's "replace a child": validate, then put `new` where `old` was. */
fn replace_child(vm: &mut Vm, parent: usize, new: usize, old: usize) -> Res<()> {
    ensure_pre_insert(vm, parent, new, Some(old), true)?;
    if new == old {
        return Ok(());
    }
    let mut reference = sibling(vm, old, 1, false);
    if reference == Some(new) {
        reference = sibling(vm, new, 1, false);
    }
    remove_node(vm, old)?;
    insert(vm, parent, new, reference)
}

/**
 * Take `node` out of the parent it is in before an insertion. A fragment's children leave the
 * fragment silently in the child list log (the fragment itself was logged), but still move their
 * live range points out as a removal does.
 */
fn leave_current_parent(vm: &mut Vm, node: usize, from_fragment: bool) -> Res<()> {
    let Some(old) = vm.page.parent(node) else {
        return Ok(());
    };
    if !from_fragment {
        children_changed(vm, old, Vec::new(), vec![node]);
    }
    let index = index_of(vm, old, node);
    range_live::removed(vm, old, index, node)
}

/** The child index an insertion before `reference` (or at the end) lands at. */
fn insertion_index(vm: &mut Vm, parent: usize, reference: Option<usize>) -> usize {
    let siblings = vm.page.children(parent);
    reference
        .and_then(|reference| siblings.iter().position(|id| *id == reference))
        .unwrap_or(siblings.len())
}

/** Insert a node (fragments insert their children), adopting new identities. */
pub(in crate::v9) fn insert(
    vm: &mut Vm,
    parent: usize,
    child: usize,
    reference: Option<usize>,
) -> Res<()> {
    if parent == DOCUMENT {
        return Err(vm.unsupported_error("inserting directly into the document"));
    }
    let tag = vm.page.tag(child);
    if tag == components::SHADOW_ROOT {
        return Err(vm.throw(
            crate::v9::vm::error::ErrorKind::Error,
            "HierarchyRequestError: A shadow root cannot be inserted.",
        ));
    }
    let children = if tag == components::FRAGMENT {
        vm.page.children(child)
    } else {
        vec![child]
    };
    if tag == components::FRAGMENT && !children.is_empty() {
        children_changed(vm, child, Vec::new(), children.clone());
    }
    let mut added = Vec::with_capacity(children.len());
    for node in children {
        leave_current_parent(vm, node, tag == components::FRAGMENT)?;
        let index = insertion_index(vm, parent, reference);
        let mapping = vm
            .page
            .insert(parent, node, reference)
            .map_err(|error| dom_error(vm, error))?;
        range_live::inserted(vm, parent, index, 1)?;
        adopt(vm, &mapping)?;
        added.push(
            mapping
                .iter()
                .find(|(old, _)| *old == node)
                .map_or(node, |(_, new)| *new),
        );
    }
    for node in &added {
        super::scripts::after_insert(vm, *node)?;
    }
    let siblings = vm.page.children(parent);
    let first = added
        .first()
        .and_then(|first| siblings.iter().position(|c| c == first));
    let last = added
        .last()
        .and_then(|last| siblings.iter().position(|c| c == last));
    vm.page.mutations.push(components::Mutation::Children {
        parent,
        added,
        removed: Vec::new(),
        previous: first
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| siblings.get(index).copied()),
        next: last.and_then(|index| siblings.get(index + 1).copied()),
    });
    Ok(())
}

/** Copy a node (and optionally its subtree); defined custom elements in the copy upgrade. */
pub(super) fn clone_node(vm: &mut Vm, node: usize, deep: bool) -> Res<usize> {
    let copy = clone_tree(vm, node, deep)?;
    vm.page
        .mutations
        .push(components::Mutation::Created { root: copy });
    Ok(copy)
}

/** Copy a node (and optionally its subtree, template content included) as detached nodes. */
pub(super) fn clone_tree(vm: &mut Vm, node: usize, deep: bool) -> Res<usize> {
    let Some(view) = vm.page.view(node) else {
        return Err(vm.type_error("Cannot clone this node"));
    };
    let tag = view.tag.clone();
    let text = if view.tag.starts_with('#') {
        Some(vm.page.text_content(node))
    } else {
        view.text.clone()
    };
    let copy = vm
        .page
        .create(&tag, text)
        .map_err(|error| dom_error(vm, error))?;
    if let Some(record) = vm.page.state.detached.get_mut(&copy) {
        record.attributes = view.attributes.clone();
    }
    vm.page.copy_namespaces(node, copy);
    super::frames::copy_document_kind(vm, node, copy);
    if let Some(value) = view.value {
        vm.page.state.values.insert(copy, value);
    }
    if deep {
        for child in vm.page.children(node) {
            let child_copy = clone_tree(vm, child, true)?;
            vm.page
                .insert(copy, child_copy, None)
                .map_err(|error| dom_error(vm, error))?;
        }
        components::clone_content(vm, node, copy)?;
    }
    Ok(copy)
}

/** Element accessors. */
pub(in crate::v9) fn element_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = this_node(vm, &call)?;
    let name = native_name(&call);
    let view = vm.page.view(node);
    let attribute = |attribute: &str| {
        view.as_ref()
            .and_then(|view| view.attributes.get(attribute).cloned())
    };
    match name {
        "tagName" => JsValue::string(&qualified_name(vm, node)).map_err(Into::into),
        "localName" => {
            let local = vm.page.element_local_name(node);
            JsValue::string(&local).map_err(Into::into)
        }
        "namespaceURI" => optional_string(vm.page.element_namespace(node)),
        "prefix" => optional_string(vm.page.element_prefix(node)),
        "className" => JsValue::string(&attribute("class").unwrap_or_default()).map_err(Into::into),
        "children" => live_children(vm, node, true),
        "childElementCount" => {
            let count = traverse_children(vm, node)
                .into_iter()
                .filter(|child| vm.page.is_element(*child))
                .count();
            Ok(JsValue::Number(count as f64))
        }
        "firstElementChild" => {
            let first = traverse_children(vm, node)
                .into_iter()
                .find(|child| vm.page.is_element(*child));
            wrap_option(vm, first)
        }
        "lastElementChild" => {
            let last = traverse_children(vm, node)
                .into_iter()
                .rev()
                .find(|child| vm.page.is_element(*child));
            wrap_option(vm, last)
        }
        "nextElementSibling" => {
            let next = sibling(vm, node, 1, true);
            wrap_option(vm, next)
        }
        "previousElementSibling" => {
            let previous = sibling(vm, node, -1, true);
            wrap_option(vm, previous)
        }
        "innerHTML" => {
            let html = serialize(&mut vm.page, node, false);
            JsValue::string(&html).map_err(Into::into)
        }
        "outerHTML" => {
            let html = serialize(&mut vm.page, node, true);
            JsValue::string(&html).map_err(Into::into)
        }
        "classList" => token_list_object(vm, node, "class"),
        "dataset" => host_object(vm, HostData::Dataset(node), realm().ids.dataset_prototype),
        "sheet" => super::sheets::element_sheet(vm, node),
        "contentDocument" | "contentWindow" => super::frames::frame_get(vm, node, name),
        "sandbox" | "relList" | "part" => {
            let attribute = if name == "relList" { "rel" } else { name };
            token_list_object(vm, node, attribute)
        }
        "pathname" | "host" | "hostname" | "protocol" | "search" | "hash" | "origin" | "port"
        | "username" | "password" => {
            // Links decompose their resolved href as `URL` does; an unresolvable one is empty.
            let href = href_in_document_encoding(vm, &attribute("href").unwrap_or_default());
            let resolved = super::net::resolve(vm, &href);
            let parsed = resolved
                .as_deref()
                .and_then(|url| url::Url::parse(url).ok());
            let Some(url) = parsed else {
                return JsValue::string("").map_err(Into::into);
            };
            let text = match name {
                "pathname" => url.path().to_string(),
                "host" => url
                    .host_str()
                    .map_or(String::new(), |host| match url.port() {
                        Some(port) => format!("{host}:{port}"),
                        None => host.to_string(),
                    }),
                "hostname" => url.host_str().unwrap_or_default().to_string(),
                "protocol" => format!("{}:", url.scheme()),
                "search" => url
                    .query()
                    .map_or(String::new(), |query| format!("?{query}")),
                "hash" => url
                    .fragment()
                    .map_or(String::new(), |fragment| format!("#{fragment}")),
                "origin" => url.origin().ascii_serialization(),
                "port" => url.port().map_or(String::new(), |port| port.to_string()),
                "username" => url.username().to_string(),
                _ => url.password().unwrap_or_default().to_string(),
            };
            JsValue::string(&text).map_err(Into::into)
        }
        "validity" => super::sheets::validity(vm),
        "willValidate" => Ok(JsValue::Boolean(true)),
        "validationMessage" => JsValue::string("").map_err(Into::into),
        "style" => host_object(vm, HostData::Style(node), realm().ids.style_prototype),
        "attributes" => attr_dom::named_node_map(vm, node),
        "value" => {
            let value = view
                .as_ref()
                .and_then(|view| view.value.clone())
                .or_else(|| attribute("value"));
            match value {
                Some(value) => JsValue::string(&value).map_err(Into::into),
                None if vm.page.tag(node) == "select" => {
                    let options = vm.page.descendants(node);
                    let selected = options.into_iter().find(|option| {
                        vm.page.view(*option).is_some_and(|view| {
                            view.tag == "option" && view.attributes.contains_key("selected")
                        })
                    });
                    let text = selected
                        .map(|option| {
                            vm.page
                                .view(option)
                                .and_then(|view| view.attributes.get("value").cloned())
                                .unwrap_or_else(|| vm.page.text_content(option))
                        })
                        .unwrap_or_default();
                    JsValue::string(&text).map_err(Into::into)
                }
                None if vm.page.tag(node) == "textarea" || vm.page.tag(node) == "option" => {
                    let text = vm.page.text_content(node);
                    JsValue::string(&text).map_err(Into::into)
                }
                None => JsValue::string("").map_err(Into::into),
            }
        }
        "scrollWidth" if node == HTML || node == BODY => {
            let width = super::scrolling::document_width(vm);
            let fallback = client_box(vm, node).2;
            Ok(JsValue::Number(width.unwrap_or(fallback)))
        }
        "offsetWidth" | "offsetHeight" | "clientWidth" | "clientHeight" | "scrollWidth"
        | "scrollHeight" => {
            let (_, _, width, height) = client_box(vm, node);
            let laid_out = !vm.page.environment.geometry.is_empty();
            let size = if name.ends_with("Width") {
                width
            } else {
                height
            };
            let size = if size > 0.0 || laid_out || node == HTML || node == BODY {
                size
            } else if name.starts_with("offset") && rendered(vm, node) {
                // Before the first layout, rendered elements report a nominal box, hidden ones zero.
                NOMINAL_BOX
            } else {
                0.0
            };
            Ok(JsValue::Number(size))
        }
        "offsetTop" | "offsetLeft" => {
            let axis = usize::from(name == "offsetTop");
            Ok(JsValue::Number(super::offsets::offset(vm, node, axis)))
        }
        "scrollTop" => Ok(JsValue::Number(super::scrolling::offset(vm, node)[1])),
        "scrollLeft" => Ok(JsValue::Number(super::scrolling::offset(vm, node)[0])),
        "clientTop" | "clientLeft" => Ok(JsValue::Number(0.0)),
        "offsetParent" => match super::offsets::offset_parent(vm, node) {
            Some(parent) => wrap(vm, parent),
            None => Ok(JsValue::Null),
        },
        "elements" if vm.page.tag(node) == "form" => {
            let controls = super::form_submit::controls(vm, node);
            node_list(vm, controls)
        }
        "form" => {
            let form = vm
                .page
                .ancestors(node)
                .into_iter()
                .find(|ancestor| vm.page.tag(*ancestor) == "form");
            wrap_option(vm, form)
        }
        "options" => {
            let options: Vec<usize> = vm
                .page
                .descendants(node)
                .into_iter()
                .filter(|child| vm.page.tag(*child) == "option")
                .collect();
            node_list(vm, options)
        }
        "selectedIndex" => {
            let options: Vec<usize> = vm
                .page
                .descendants(node)
                .into_iter()
                .filter(|child| vm.page.tag(*child) == "option")
                .collect();
            let index = options.iter().position(|option| {
                vm.page
                    .view(*option)
                    .is_some_and(|view| view.attributes.contains_key("selected"))
            });
            Ok(JsValue::Number(match index {
                Some(index) => index as f64,
                None if options.is_empty() => -1.0,
                None => 0.0,
            }))
        }
        "text" => {
            let text = vm.page.text_content(node);
            JsValue::string(&text).map_err(Into::into)
        }
        _ => Ok(JsValue::Undefined),
    }
}

/** A cached-per-call host helper object (classList, dataset, style). */
fn host_object(vm: &mut Vm, data: HostData, proto: ObjectId) -> Res<JsValue> {
    let id = vm.alloc(Object::new(Class::Host(Box::new(data)), Some(proto)))?;
    Ok(JsValue::Object(id))
}

/** Element setters. */
/** Runs the steps below, then the custom element reactions and records they queued. */
pub(in crate::v9) fn element_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| element_set_steps(vm, call))
}

/** The element_set steps themselves. */
fn element_set_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = this_node(vm, &call)?;
    let value = call.arg(0);
    match native_name(&call) {
        "className" => {
            let text = vm.to_text(&value)?;
            vm.page
                .set_attribute(node, "class", Some(&text))
                .map_err(|error| dom_error(vm, error))?;
        }
        // Assigning a token list sets its attribute, as the IDL's PutForwards=value says.
        name @ ("sandbox" | "relList" | "part" | "classList") => {
            let attribute = match name {
                "relList" => "rel",
                "classList" => "class",
                other => other,
            };
            let text = vm.to_text(&value)?;
            vm.page
                .set_attribute(node, attribute, Some(&text))
                .map_err(|error| dom_error(vm, error))?;
        }
        "innerHTML" => {
            let html = null_to_empty(vm, &value)?;
            set_inner_html(vm, node, &html)?;
        }
        "outerHTML" => set_outer_html(vm, node, &value)?,
        "value" => {
            let text = if value.nullish() {
                String::new()
            } else {
                vm.to_text(&value)?
            };
            vm.page
                .set_value(node, &text)
                .map_err(|error| dom_error(vm, error))?;
        }
        "style" => {
            let text = vm.to_text(&value)?;
            vm.page
                .set_attribute(node, "style", Some(&text))
                .map_err(|error| dom_error(vm, error))?;
        }
        name @ ("scrollTop" | "scrollLeft") => {
            super::scrolling::set_property(vm, node, name, &value)?;
        }
        "selectedIndex" => {}
        _ => {}
    }
    Ok(JsValue::Undefined)
}

/** LegacyNullToEmptyString for innerHTML and outerHTML: null is "", while undefined stringifies as "undefined". */
fn null_to_empty(vm: &mut Vm, value: &JsValue) -> Res<String> {
    if matches!(value, JsValue::Null) {
        return Ok(String::new());
    }
    vm.to_text(value)
}

/** The outerHTML setter: a parentless element has nowhere to be replaced, so the setter leaves it alone. */
fn set_outer_html(vm: &mut Vm, node: usize, value: &JsValue) -> Res<()> {
    let html = null_to_empty(vm, value)?;
    if vm.page.parent(node).is_none() {
        // The document element's parent is the document, which cannot be replaced, so it throws.
        if node == HTML {
            return Err(vm.dom_exception(
                "NoModificationAllowedError",
                "The document element cannot be replaced.",
            ));
        }
        return Ok(());
    }
    vm.page
        .insert_html(node, InsertPosition::BeforeBegin, &html)
        .map_err(|error| dom_error(vm, error))?;
    remove_node(vm, node)
}

/** Replace a node's children with parsed HTML (a template's content's, for a template). */
pub(in crate::v9) fn set_inner_html(vm: &mut Vm, node: usize, html: &str) -> Res<()> {
    let node = if vm.page.tag(node) == "template" {
        vm.page
            .content_or_self(node)
            .map_err(|error| dom_error(vm, error))?
    } else {
        node
    };
    let removed = vm.page.children(node);
    clear_children_hooks(vm, node)?;
    replace_html(vm, node, html)?;
    let added = vm.page.children(node);
    for child in &added {
        vm.page
            .mutations
            .push(components::Mutation::Created { root: *child });
    }
    children_changed(vm, node, added, removed);
    Ok(())
}

/** Replace a node's children with parsed HTML in whichever store holds it. */
fn replace_html(vm: &mut Vm, node: usize, html: &str) -> Res<()> {
    let host = vm.page.has_shadows() && vm.page.shadow_root(node).is_some();
    if vm.page.is_projected(node) && !host {
        vm.page
            .set_text(node, "")
            .map_err(|error| dom_error(vm, error))?;
        if !html.is_empty() {
            vm.page
                .insert_html(node, InsertPosition::BeforeEnd, html)
                .map_err(|error| dom_error(vm, error))?;
        }
        return Ok(());
    }
    for child in vm.page.children(node) {
        vm.page
            .remove(child)
            .map_err(|error| dom_error(vm, error))?;
    }
    vm.page
        .insert_html(node, InsertPosition::BeforeEnd, html)
        .map_err(|error| dom_error(vm, error))
}

/**
 * NamedNodeMap's named getter: `attributes['data-x']` is the Attr, as WebIDL's
 * [LegacyUnenumerableNamedProperties] gives it, so the names are not enumerable and never
 * shadow an index or a member on the chain (`length`, `item`). jQuery 1.x's feature probe reads
 * `div.attributes['onfocusin'].expando` and throws on load without it.
 */
pub(super) fn named_attributes(
    vm: &mut Vm,
    list: ObjectId,
    named: Vec<(String, ObjectId)>,
) -> Res<()> {
    for (name, item) in named {
        let key = Key::from(name.as_str());
        if key.index().is_some() || vm.has_property(list, &key)? {
            continue;
        }
        let desc = crate::v9::vm::property::Descriptor {
            value: Some(JsValue::Object(item)),
            writable: Some(false),
            enumerable: Some(false),
            configurable: Some(true),
            ..Default::default()
        };
        vm.define(list, key, desc)?;
    }
    Ok(())
}

/** NamedNodeMap lookups over the Attr records `attributes` built; the map is a snapshot, so
writes through it are unsupported. */
pub(in crate::v9) fn named_node_map_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = native_name(&call);
    let length = vm.get(&call.this, &Key::from("length"))?;
    let count = vm.to_integer(&length)?.max(0.0) as usize;
    match name {
        "item" => {
            let index = vm.to_integer(&call.arg(0))?;
            if index < 0.0 || index as usize >= count {
                return Ok(JsValue::Null);
            }
            vm.get(&call.this, &Key::from(index.to_string().as_str()))
        }
        "getNamedItem" => {
            let wanted = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
            for index in 0..count {
                let item = vm.get(&call.this, &Key::from(index.to_string().as_str()))?;
                let name = vm.get(&item, &Key::from("name"))?;
                if vm.to_text(&name)? == wanted {
                    return Ok(item);
                }
            }
            Ok(JsValue::Null)
        }
        _ => Err(vm.unsupported_error("NamedNodeMap writes")),
    }
}

/** Reflected attributes: the accessor's own name says which attribute it mirrors. */
pub(in crate::v9) fn reflect_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    // V9 hangs every reflected attribute on Element.prototype, where browsers have only some on
    // each interface; read through a non-node (a class prototype a component library walks),
    // the attribute is simply absent.
    let Some(node) = node_of(vm, &call.this) else {
        return Ok(JsValue::Undefined);
    };
    let name = native_name(&call);
    let (attribute, kind) = reflected(name);
    let value = vm
        .page
        .view(node)
        .and_then(|view| view.attributes.get(attribute).cloned());
    match kind {
        Reflect::Boolean => Ok(JsValue::Boolean(value.is_some())),
        Reflect::Url => {
            let Some(value) = value else {
                return JsValue::string("").map_err(Into::into);
            };
            let resolved = super::net::resolve(vm, &value).unwrap_or(value);
            JsValue::string(&resolved).map_err(Into::into)
        }
        Reflect::Number => Ok(JsValue::Number(
            value
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(if name == "tabIndex" { -1.0 } else { 0.0 }),
        )),
        Reflect::Text => JsValue::string(&value.unwrap_or_default()).map_err(Into::into),
    }
}

/** Reflected attribute setters. */
/** Runs the steps below, then the custom element reactions and records they queued. */
pub(in crate::v9) fn reflect_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| reflect_set_steps(vm, call))
}

/** The reflect_set steps themselves. */
fn reflect_set_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(node) = node_of(vm, &call.this) else {
        // On a non-node the inherited attribute is absent, so assignment makes a data property.
        if let Some(id) = call.this.object() {
            vm.put(id, native_name(&call), call.arg(0))?;
        }
        return Ok(JsValue::Undefined);
    };
    let (attribute, kind) = reflected(native_name(&call));
    let value = call.arg(0);
    let result = match kind {
        Reflect::Boolean => {
            if value.truthy() {
                vm.page.set_attribute(node, attribute, Some(""))
            } else {
                vm.page.set_attribute(node, attribute, None)
            }
        }
        _ => {
            let text = vm.to_text(&value)?;
            vm.page.set_attribute(node, attribute, Some(&text))
        }
    };
    result.map_err(|error| dom_error(vm, error))?;
    if attribute == "src" {
        super::scripts::after_insert(vm, node)?;
    } else if attribute == "media" {
        super::scripts::media_changed(vm, node);
    }
    Ok(JsValue::Undefined)
}

/** How a reflected attribute converts. */
#[derive(Clone, Copy)]
enum Reflect {
    Text,
    Boolean,
    Url,
    Number,
}

/** Reflected property names, their attributes and kinds. */
pub(in crate::v9) const REFLECTED: [&str; 33] = [
    "id",
    "title",
    "lang",
    "dir",
    "name",
    "type",
    "alt",
    "rel",
    "target",
    "placeholder",
    "htmlFor",
    "accessKey",
    "role",
    "hidden",
    "checked",
    "selected",
    "disabled",
    "readOnly",
    "required",
    "multiple",
    "autofocus",
    "defer",
    "async",
    "href",
    "src",
    "action",
    "cite",
    "data",
    "tabIndex",
    "width",
    "height",
    "maxLength",
    "colSpan",
];

/** The attribute behind a reflected property. */
fn reflected(name: &str) -> (&str, Reflect) {
    match name {
        "htmlFor" => ("for", Reflect::Text),
        "accessKey" => ("accesskey", Reflect::Text),
        "readOnly" => ("readonly", Reflect::Boolean),
        "maxLength" => ("maxlength", Reflect::Number),
        "colSpan" => ("colspan", Reflect::Number),
        "tabIndex" => ("tabindex", Reflect::Number),
        "width" | "height" => (name, Reflect::Number),
        "hidden" | "checked" | "selected" | "disabled" | "required" | "multiple" | "autofocus"
        | "defer" | "async" => (name, Reflect::Boolean),
        "href" | "src" | "action" | "cite" | "data" => (name, Reflect::Url),
        _ => (name, Reflect::Text),
    }
}

/** Element methods. */
/** A JavaScript string for a present value, or null for an absent one. */
fn optional_string(value: Option<String>) -> Res<JsValue> {
    match value {
        Some(text) => JsValue::string(&text).map_err(Into::into),
        None => Ok(JsValue::Null),
    }
}

/** A string argument read as an optional one: null, undefined and the empty string all mean
none, which is how the namespace and prefix arguments of the DOM read them. */
pub(super) fn optional_string_arg(vm: &mut Vm, call: &Call, index: usize) -> Res<Option<String>> {
    let value = call.arg(index);
    if value.nullish() {
        return Ok(None);
    }
    let text = vm.to_text(&value)?;
    Ok(namespaces::non_empty(&text))
}

/** Turns a rejected name into the DOMException the DOM standard names for it. */
pub(super) fn name_error(vm: &mut Vm, error: NameError) -> Thrown {
    vm.dom_exception(error.name, &error.message)
}

/** The attribute a namespace-aware read names: the namespace and local name arguments find the
stored name, and the POPOVER marker, an engine-only attribute, is never reported. */
fn namespaced_attribute(vm: &mut Vm, node: usize, call: &Call) -> Res<Option<(String, String)>> {
    let namespace = optional_string_arg(vm, call, 0)?;
    let local = vm.to_text(&call.arg(1))?;
    let Some(name) = vm
        .page
        .namespaced_attribute_name(node, namespace.as_deref(), &local)
    else {
        return Ok(None);
    };
    if name == crate::document::POPOVER_OPEN_ATTRIBUTE {
        return Ok(None);
    }
    Ok(vm
        .page
        .attribute_value(node, &name)
        .map(|value| (name, value)))
}

/** setAttributeNS: the name is validated and extracted, then stored with its namespace, keeping
its case, since a namespace-aware write is not lowercased the way setAttribute's is. */
fn set_namespaced_attribute(vm: &mut Vm, node: usize, call: &Call) -> Res<JsValue> {
    let namespace = optional_string_arg(vm, call, 0)?;
    let qualified = vm.to_text(&call.arg(1))?;
    let value = vm.to_text(&call.arg(2))?;
    let qname =
        namespaces::validate_and_extract(namespace.as_deref(), &qualified, NameKind::Attribute)
            .map_err(|error| name_error(vm, error))?;
    vm.page
        .set_namespaced_attribute(node, namespace.as_deref(), &qname, &qualified, &value)
        .map_err(|error| dom_error(vm, error))?;
    Ok(JsValue::Undefined)
}

/** Runs the steps below, then the custom element reactions and records they queued. */
pub(in crate::v9) fn element_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| element_method_steps(vm, call))
}

/** The element_method steps themselves. */
fn element_method_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = this_node(vm, &call)?;
    match native_name(&call) {
        "setAttributeNS" => set_namespaced_attribute(vm, node, &call),
        "getAttributeNS" => match namespaced_attribute(vm, node, &call)? {
            Some((_, value)) => JsValue::string(&value).map_err(Into::into),
            None => Ok(JsValue::Null),
        },
        "hasAttributeNS" => Ok(JsValue::Boolean(
            namespaced_attribute(vm, node, &call)?.is_some(),
        )),
        "removeAttributeNS" => {
            if let Some((name, _)) = namespaced_attribute(vm, node, &call)? {
                vm.page
                    .set_named_attribute(node, &name, None)
                    .map_err(|error| dom_error(vm, error))?;
            }
            Ok(JsValue::Undefined)
        }
        "getAttribute" => {
            let name = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
            match vm
                .page
                .view(node)
                .and_then(|view| view.attributes.get(&name).cloned())
            {
                Some(value) => JsValue::string(&value).map_err(Into::into),
                None => Ok(JsValue::Null),
            }
        }
        "setAttribute" => {
            let name = vm.to_text(&call.arg(0))?;
            let value = vm.to_text(&call.arg(1))?;
            vm.page
                .set_attribute(node, &name, Some(&value))
                .map_err(|error| dom_error(vm, error))?;
            if name.eq_ignore_ascii_case("src") {
                super::scripts::after_insert(vm, node)?;
            } else if name.eq_ignore_ascii_case("media") {
                super::scripts::media_changed(vm, node);
            }
            Ok(JsValue::Undefined)
        }
        "removeAttribute" => {
            let name = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
            let present = vm
                .page
                .view(node)
                .is_some_and(|view| view.attributes.contains_key(&name));
            if present {
                vm.page
                    .set_attribute(node, &name, None)
                    .map_err(|error| dom_error(vm, error))?;
            }
            Ok(JsValue::Undefined)
        }
        "hasAttribute" => {
            let name = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
            Ok(JsValue::Boolean(
                vm.page
                    .view(node)
                    .is_some_and(|view| view.attributes.contains_key(&name)),
            ))
        }
        "hasAttributes" => Ok(JsValue::Boolean(
            vm.page
                .view(node)
                .is_some_and(|view| !view.attributes.is_empty()),
        )),
        "getAttributeNames" => {
            let names: Vec<JsValue> = vm
                .page
                .view(node)
                .map(|view| {
                    view.attributes
                        .keys()
                        .filter_map(|name| JsValue::string(name).ok())
                        .collect()
                })
                .unwrap_or_default();
            Ok(JsValue::Object(vm.new_array(names)?))
        }
        "toggleAttribute" => {
            let name = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
            let present = vm
                .page
                .view(node)
                .is_some_and(|view| view.attributes.contains_key(&name));
            let wanted = if call.args.len() > 1 {
                call.arg(1).truthy()
            } else {
                !present
            };
            if wanted != present {
                let value = if wanted { Some("") } else { None };
                vm.page
                    .set_attribute(node, &name, value)
                    .map_err(|error| dom_error(vm, error))?;
            }
            Ok(JsValue::Boolean(wanted))
        }
        "getAttributeNode"
        | "getAttributeNodeNS"
        | "setAttributeNode"
        | "setAttributeNodeNS"
        | "removeAttributeNode" => attr_dom::element_attr_method(vm, node, &call),
        "querySelector" | "querySelectorAll" => query(vm, node, &call),
        "getElementsByTagName" => elements_by_tag(vm, node, &call),
        "getElementsByClassName" => elements_by_class(vm, node, &call),
        "matches" | "webkitMatchesSelector" | "msMatchesSelector" => {
            let text = vm.to_text(&call.arg(0))?;
            let list = parse_selector(vm, "matches", &text)?;
            Ok(JsValue::Boolean(selector::matches(
                &mut vm.page,
                &vm.heap,
                node,
                &list,
                Some(node),
            )))
        }
        "closest" => {
            let text = vm.to_text(&call.arg(0))?;
            let list = parse_selector(vm, "closest", &text)?;
            let mut candidates = vec![node];
            candidates.extend(vm.page.ancestors(node));
            let found = candidates
                .into_iter()
                .find(|id| selector::matches(&mut vm.page, &vm.heap, *id, &list, Some(node)));
            wrap_option(vm, found)
        }
        "remove" => {
            if vm.page.parent(node).is_some() || vm.page.state.head.contains(&node) {
                remove_node(vm, node)?;
            }
            Ok(JsValue::Undefined)
        }
        "append" | "prepend" | "before" | "after" | "replaceWith" | "replaceChildren" => {
            mutate(vm, node, &call)
        }
        "insertAdjacentHTML" => {
            let position = position(vm, &call.arg(0))?;
            let html = vm.to_text(&call.arg(1))?;
            vm.page
                .insert_html(node, position, &html)
                .map_err(|error| dom_error(vm, error))?;
            Ok(JsValue::Undefined)
        }
        "insertAdjacentElement" | "insertAdjacentText" => {
            let position = position(vm, &call.arg(0))?;
            let inserted = if native_name(&call) == "insertAdjacentText" {
                let text = vm.to_text(&call.arg(1))?;
                vm.page
                    .create("#text", Some(text))
                    .map_err(|error| dom_error(vm, error))?
            } else {
                arg_node(vm, &call, 1)?
            };
            adjacent(vm, node, position, inserted)?;
            Ok(call.arg(1))
        }
        "getBoundingClientRect" => rect(vm, node),
        "getClientRects" => {
            // Without layout, a rendered element reports one (empty) box so visibility checks pass.
            let rects = if rendered(vm, node) {
                vec![rect(vm, node)?]
            } else {
                Vec::new()
            };
            Ok(JsValue::Object(vm.new_array(rects)?))
        }
        "focus" => {
            focus(vm, node)?;
            Ok(JsValue::Undefined)
        }
        "blur" => {
            if vm.page.state.focused == Some(node) {
                blur(vm)?;
            }
            Ok(JsValue::Undefined)
        }
        "load" | "pause" => Ok(JsValue::Undefined),
        "play" => {
            // Nothing plays, so playback "starts" at once; the media element stays paused.
            let promise = vm.promise_resolve(JsValue::Undefined)?;
            Ok(JsValue::Object(promise))
        }
        "canPlayType" => JsValue::string("").map_err(Into::into),
        name @ ("scrollTo" | "scrollBy" | "scroll") => {
            super::scrolling::scroll_method(vm, node, &call, name == "scrollBy")?;
            Ok(JsValue::Undefined)
        }
        "scrollIntoView" | "setPointerCapture" | "releasePointerCapture" | "select" => {
            Ok(JsValue::Undefined)
        }
        "click" => {
            if super::events::fire(vm, super::Target::Node(node), "click", true, true)? {
                super::interfaces::popover::activate(vm, node)?;
            }
            Ok(JsValue::Undefined)
        }
        "checkValidity" | "reportValidity" => Ok(JsValue::Boolean(true)),
        "setCustomValidity" => Ok(JsValue::Undefined),
        "submit" | "requestSubmit" if vm.page.tag(node) == "form" => {
            super::form_submit::submit(vm, node, &call, native_name(&call) == "requestSubmit")
        }
        "submit" | "requestSubmit" | "reset" => {
            Err(vm.unsupported_error("form submission from script"))
        }
        "attachShadow" => components::attach_shadow(vm, node, &call.arg(0)),
        "requestFullscreen" => Err(vm.unsupported_error(native_name(&call))),
        _ => Ok(JsValue::Undefined),
    }
}

/** insertAdjacent* positions. */
fn position(vm: &mut Vm, value: &JsValue) -> Res<InsertPosition> {
    let text = vm.to_text(value)?.to_ascii_lowercase();
    Ok(match text.as_str() {
        "beforebegin" => InsertPosition::BeforeBegin,
        "afterbegin" => InsertPosition::AfterBegin,
        "beforeend" => InsertPosition::BeforeEnd,
        "afterend" => InsertPosition::AfterEnd,
        _ => return Err(vm.dom_exception("SyntaxError", &format!("The value provided ('{text}') is not one of 'beforeBegin', 'afterBegin', 'beforeEnd', or 'afterEnd'."))),
    })
}

/** Insert a node relative to another. */
fn adjacent(vm: &mut Vm, node: usize, position: InsertPosition, inserted: usize) -> Res<()> {
    match position {
        InsertPosition::AfterBegin => {
            let first = vm.page.children(node).first().copied();
            insert(vm, node, inserted, first)
        }
        InsertPosition::BeforeEnd => insert(vm, node, inserted, None),
        InsertPosition::BeforeBegin => {
            let parent = vm
                .page
                .parent(node)
                .ok_or_else(|| vm.type_error("The element has no parent."))?;
            insert(vm, parent, inserted, Some(node))
        }
        InsertPosition::AfterEnd => {
            let parent = vm
                .page
                .parent(node)
                .ok_or_else(|| vm.type_error("The element has no parent."))?;
            let next = sibling(vm, node, 1, false);
            insert(vm, parent, inserted, next)
        }
    }
}

/** Nodes for append/prepend/before/after arguments (strings become text nodes). */
fn argument_nodes(vm: &mut Vm, call: &Call) -> Res<Vec<usize>> {
    let mut nodes = Vec::new();
    for value in call.args {
        match node_of(vm, value) {
            Some(node) => nodes.push(node),
            None => {
                let text = vm.to_text(value)?;
                nodes.push(
                    vm.page
                        .create("#text", Some(text))
                        .map_err(|error| dom_error(vm, error))?,
                );
            }
        }
    }
    Ok(nodes)
}

/** append, prepend, before, after, replaceWith, replaceChildren, as the DOM standard's
ParentNode and ChildNode steps: the nodes are validated first, then moved out of wherever they
were (as "convert nodes into a node" does with its fragment), then inserted together at a
point found among the siblings that are not being moved. */
fn mutate(vm: &mut Vm, node: usize, call: &Call) -> Res<JsValue> {
    let name = native_name(call);
    let nodes = argument_nodes(vm, call)?;
    let child_steps = matches!(name, "before" | "after" | "replaceWith");
    let parent = if child_steps {
        match traverse_parent(vm, node) {
            Some(parent) => parent,
            None => return Ok(JsValue::Undefined),
        }
    } else {
        node
    };
    for child in &nodes {
        ensure_pre_insert(vm, parent, *child, None, false)?;
    }
    let viable = |vm: &mut Vm, offset: isize| {
        let mut at = sibling(vm, node, offset, false);
        while let Some(candidate) = at.filter(|c| nodes.contains(c)) {
            at = sibling(vm, candidate, offset, false);
        }
        at
    };
    let previous = viable(vm, -1);
    let next = viable(vm, 1);
    // The first child of `parent` from `start` on that is not being moved: inserting before it
    // lands where the standard's fragment would, without detaching projected nodes first.
    let unmoved = |vm: &mut Vm, start: Option<usize>| {
        let mut at = start;
        while let Some(candidate) = at.filter(|c| nodes.contains(c)) {
            at = sibling(vm, candidate, 1, false);
        }
        at
    };
    let reference = match name {
        "append" => None,
        "prepend" => {
            let first = traverse_children(vm, node).first().copied();
            // When the first child is also the first node prepended it keeps its place; the
            // insert treats "before itself" as that, and the change log stays an afterbegin.
            if first.is_some() && first == nodes.first().copied() {
                first
            } else {
                unmoved(vm, first)
            }
        }
        "before" => {
            let start = match previous {
                Some(previous) => sibling(vm, previous, 1, false),
                None => traverse_children(vm, parent).first().copied(),
            };
            unmoved(vm, start)
        }
        "after" | "replaceWith" => next,
        _ => {
            for child in traverse_children(vm, node) {
                if !nodes.contains(&child) {
                    remove_node(vm, child)?;
                }
            }
            None
        }
    };
    if name == "replaceWith" && !nodes.contains(&node) {
        remove_node(vm, node)?;
    }
    for child in nodes {
        insert(vm, parent, child, reference)?;
    }
    Ok(JsValue::Undefined)
}

/** Descendants matching a predicate, in document order. */
pub(super) fn search(
    vm: &mut Vm,
    root: usize,
    mut predicate: impl FnMut(&mut super::Page, usize) -> bool,
) -> Vec<usize> {
    let all = if root == DOCUMENT {
        let mut all = vec![HTML, HEAD];
        all.extend(vm.page.descendants(HEAD));
        all.push(BODY);
        all.extend(vm.page.descendants(BODY));
        all
    } else {
        vm.page.descendants(root)
    };
    all.into_iter()
        .filter(|id| predicate(&mut vm.page, *id))
        .collect()
}

/** Parse a selector for `method`. A bad selector is the DOM's SyntaxError DOMException (code 12),
which scripts test by name, not the JavaScript SyntaxError class. */
fn parse_selector(vm: &mut Vm, method: &str, text: &str) -> Res<selector::SelectorList> {
    selector::parse(text).map_err(|message| {
        vm.dom_exception(
            "SyntaxError",
            &format!("Failed to execute '{method}' on 'Document': {message}"),
        )
    })
}

/** getElementsByTagName for an element or the document. The document searches its whole tree,
including the root element, which an element's own search never includes. */
fn elements_by_tag(vm: &mut Vm, node: usize, call: &Call) -> Res<JsValue> {
    let tag = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
    let found = search(vm, node, |page, id| {
        page.is_element(id) && (tag == "*" || page.tag(id) == tag)
    });
    node_list(vm, found)
}

/** getElementsByClassName: the elements under `node` carrying every named class. An empty list of
class names matches nothing, as the DOM requires. */
fn elements_by_class(vm: &mut Vm, node: usize, call: &Call) -> Res<JsValue> {
    let wanted: Vec<String> = vm
        .to_text(&call.arg(0))?
        .split_ascii_whitespace()
        .map(String::from)
        .collect();
    let found = search(vm, node, |page, id| {
        let classes = page
            .view(id)
            .and_then(|view| view.attributes.get("class").cloned())
            .unwrap_or_default();
        !wanted.is_empty()
            && wanted
                .iter()
                .all(|class| classes.split_ascii_whitespace().any(|c| c == class))
    });
    node_list(vm, found)
}

/** querySelector and querySelectorAll. */
pub(super) fn query(vm: &mut Vm, node: usize, call: &Call) -> Res<JsValue> {
    let all = native_name(call) == "querySelectorAll";
    let text = vm.to_text(&call.arg(0))?;
    let list = parse_selector(vm, "querySelector", &text)?;
    let scope = (node != DOCUMENT).then_some(node);
    vm.charge(
        vm.page
            .document
            .as_ref()
            .map_or(0, |document| document.nodes.len()),
    )?;
    if all {
        let candidates = search(vm, node, |_, _| true);
        let found = candidates
            .into_iter()
            .filter(|id| selector::matches(&mut vm.page, &vm.heap, *id, &list, scope))
            .collect();
        return node_list(vm, found);
    }
    let candidates = search(vm, node, |_, _| true);
    let found = candidates
        .into_iter()
        .find(|id| selector::matches(&mut vm.page, &vm.heap, *id, &list, scope));
    wrap_option(vm, found)
}

/** A DOMRect-like object; V9 does not lay out, so every box is empty. */
pub(super) fn rect(vm: &mut Vm, node: usize) -> Res<JsValue> {
    let id = vm.new_object()?;
    let (x, y, width, height) = client_box(vm, node);
    for (name, value) in [
        ("x", x),
        ("y", y),
        ("top", y),
        ("left", x),
        ("width", width),
        ("height", height),
        ("right", x + width),
        ("bottom", y + height),
    ] {
        vm.put(id, name, JsValue::Number(value))?;
    }
    Ok(JsValue::Object(id))
}

/** A node's painted box in client coordinates (x, y, width, height) from the app's layout; the
viewport for the root elements, and an empty box at the origin when the app laid nothing out. */
pub(super) fn client_box(vm: &mut Vm, node: usize) -> (f64, f64, f64, f64) {
    let environment = &vm.page.environment;
    if node == HTML || node == BODY {
        return (
            -environment.scroll_x,
            -environment.scroll_y,
            environment.viewport_width,
            environment
                .geometry
                .iter()
                .map(|b| b.y + b.height)
                .fold(environment.viewport_height, f64::max),
        );
    }
    match environment.geometry.iter().find(|b| b.node == node) {
        Some(b) => (
            b.x - environment.scroll_x,
            b.y - environment.scroll_y,
            b.width,
            b.height,
        ),
        None => {
            note_stale_geometry(vm, node);
            (0.0, 0.0, 0.0, 0.0)
        }
    }
}

/** Reports a read of a connected node newer than the last layout, which had no chance to give
it a box; display:none nodes the layout saw stay unreported, so they never cost a relayout. */
fn note_stale_geometry(vm: &mut Vm, node: usize) {
    let newer = vm
        .page
        .environment
        .laid_out_before
        .is_some_and(|first| node >= first);
    if newer && !vm.page.effects.stale_geometry && vm.page.connected(node) {
        vm.page.effects.stale_geometry = true;
    }
}

/** `elementFromPoint`: the last painted (topmost) box containing a client point, if any. */
fn element_from_point(vm: &mut Vm, call: &Call) -> Res<Option<usize>> {
    let x = vm.to_number(&call.arg(0))? + vm.page.environment.scroll_x;
    let y = vm.to_number(&call.arg(1))? + vm.page.environment.scroll_y;
    let hit = vm
        .page
        .environment
        .geometry
        .iter()
        .rev()
        .find(|b| x >= b.x && x <= b.x + b.width && y >= b.y && y <= b.y + b.height)
        .map(|b| b.node);
    Ok(hit.filter(|node| vm.page.exists(*node)))
}

/** The encoding accessors every document has; only the main page's value comes from its parse. */
const CHARSET_ACCESSORS: [&str; 3] = ["characterSet", "charset", "inputEncoding"];

/** The encoding label other documents, and a page with no parse, report. */
const DEFAULT_CHARSET: &str = "UTF-8";

/** An encoding accessor: the main page reports the encoding its parser settled, frame documents UTF-8. */
fn charset_get(vm: &mut Vm, main_page: bool) -> Res<JsValue> {
    let label = match vm.page.state.charset.as_str() {
        "" => DEFAULT_CHARSET,
        _ if !main_page => DEFAULT_CHARSET,
        settled => settled,
    };
    JsValue::string(label).map_err(Into::into)
}

/** Document accessors. */
pub(in crate::v9) fn document_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let html = match super::host_data(vm, &call.this) {
        Some(HostData::Document(html)) => Some(html),
        _ => None,
    };
    if CHARSET_ACCESSORS.contains(&native_name(&call)) {
        return charset_get(vm, html.is_none());
    }
    let virtual_child = |vm: &mut Vm, index: usize| -> Option<usize> {
        let html = html?;
        vm.page.children(html).get(index).copied()
    };
    if let Some(document) = html {
        if native_name(&call) == "doctype" {
            return super::frames::doctype_of(vm, document);
        }
        return super::frames::document_get(vm, document, call);
    }
    match native_name(&call) {
        "doctype" => Ok(JsValue::Null),
        "body" => match html {
            Some(_) => {
                let body = virtual_child(vm, 1);
                wrap_option(vm, body)
            }
            None => wrap(vm, BODY),
        },
        "head" => match html {
            Some(_) => {
                let head = virtual_child(vm, 0);
                wrap_option(vm, head)
            }
            None => wrap(vm, HEAD),
        },
        "documentElement" => wrap(vm, html.unwrap_or(HTML)),
        "styleSheets" => super::sheets::style_sheets(vm),
        "readyState" => {
            let state = match vm.page.state.ready {
                super::ReadyState::Loading => "loading",
                super::ReadyState::Interactive => "interactive",
                super::ReadyState::Complete => "complete",
            };
            JsValue::string(state).map_err(Into::into)
        }
        "cookie" => JsValue::string(&vm.page.state.cookie).map_err(Into::into),
        "title" => {
            let title = vm
                .page
                .state
                .title
                .clone()
                .unwrap_or_else(|| vm.page.environment.title.clone());
            JsValue::string(&title).map_err(Into::into)
        }
        "URL" | "documentURI" | "baseURI" => {
            JsValue::string(&vm.page.state.href).map_err(Into::into)
        }
        "contentType" => JsValue::string(super::frames::HTML_TYPE).map_err(Into::into),
        "referrer" => JsValue::string(&vm.page.environment.referrer).map_err(Into::into),
        "domain" => {
            let host = super::net::parse_url(&vm.page.state.href)
                .map(|url| url.host_str().unwrap_or("").to_string())
                .unwrap_or_default();
            JsValue::string(&host).map_err(Into::into)
        }
        "activeElement" => {
            let focused = vm
                .page
                .state
                .focused
                .filter(|node| vm.page.connected(*node));
            wrap(vm, focused.unwrap_or(BODY))
        }
        "currentScript" => {
            let script = vm.page.current_script;
            wrap_option(vm, script)
        }
        "scrollingElement" => wrap(vm, HTML),
        "childNodes" => node_list(vm, vec![HTML]),
        "children" => node_list(vm, vec![HTML]),
        "firstChild" | "firstElementChild" | "lastChild" | "lastElementChild" => wrap(vm, HTML),
        "forms" | "images" | "links" | "scripts" | "anchors" | "embeds" | "plugins" => {
            let tag = match native_name(&call) {
                "forms" => "form",
                "images" => "img",
                "links" | "anchors" => "a",
                "scripts" => "script",
                _ => "embed",
            };
            let found = search(vm, DOCUMENT, |page, id| page.tag(id) == tag);
            node_list(vm, found)
        }
        _ => Ok(JsValue::Undefined),
    }
}

/** `document.all`'s members, over the document's elements in tree order: `length`,
`item(index)` and `namedItem(id or name)`. Indexing it like an array is not supported. */
pub(in crate::v9) fn document_all(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let elements = search(vm, DOCUMENT, |page, id| page.is_element(id));
    match native_name(&call) {
        "length" => Ok(JsValue::Number(elements.len() as f64)),
        "item" => {
            let index = vm.to_number(&call.arg(0))?;
            let found = (index >= 0.0 && index.fract() == 0.0)
                .then(|| elements.get(index as usize).copied())
                .flatten();
            found.map_or(Ok(JsValue::Null), |node| wrap(vm, node))
        }
        _ => {
            let key = vm.to_text(&call.arg(0))?;
            if key.is_empty() {
                return Ok(JsValue::Null);
            }
            let found = elements.into_iter().find(|id| {
                let view = vm.page.view(*id);
                view.is_some_and(|view| {
                    view.attributes.get("id") == Some(&key)
                        || view.attributes.get("name") == Some(&key)
                })
            });
            found.map_or(Ok(JsValue::Null), |node| wrap(vm, node))
        }
    }
}

/** An element's tagName: upper-case for HTML elements, as written for SVG ones (an `svg` and
what it holds, short of a foreignObject), as Chromium gives it. Icon loaders compare it. */
fn qualified_name(vm: &mut Vm, node: usize) -> String {
    let tag = vm.page.tag(node);
    // A script-created element's tag is its qualified name, and only HTML elements upper-case it.
    if let Some(record) = vm.page.state.element_ns.get(&node) {
        // Only an HTML-namespace element in an HTML document upper-cases its name; an XHTML
        // document keeps the case the script gave.
        let html_element = record.namespace.as_deref() == Some(HTML_NAMESPACE);
        let document = owning_document_id(vm, node);
        let html_document =
            super::frames::content_type_of(vm, document) == super::frames::HTML_TYPE;
        return if html_element && html_document {
            tag.to_ascii_uppercase()
        } else {
            tag
        };
    }
    let mut cursor = Some(node);
    let mut depth = 0;
    while let Some(current) = cursor {
        let name = vm.page.tag(current);
        if name == "svg" {
            return tag;
        }
        if (current != node && name.eq_ignore_ascii_case("foreignobject"))
            || depth > crate::constants::DOCUMENT_DEPTH
        {
            break;
        }
        depth += 1;
        cursor = vm.page.parent(current);
    }
    tag.to_ascii_uppercase()
}

/** Document setters (cookie, title). */
pub(in crate::v9) fn document_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = vm.to_text(&call.arg(0))?;
    match native_name(&call) {
        "cookie" => super::window::write_cookie(vm, &text)?,
        "title" => {
            vm.page.state.title = Some(text.clone());
            vm.page.effects.title = Some(text);
        }
        _ => {}
    }
    Ok(JsValue::Undefined)
}

/** Document methods. */
/** Runs the steps below, then the custom element reactions and records they queued. */
pub(in crate::v9) fn document_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| document_method_steps(vm, call))
}

/** The document_method steps themselves. */
fn document_method_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if node_of(vm, &call.this) != Some(DOCUMENT)
        && !matches!(
            super::host_data(vm, &call.this),
            Some(HostData::Document(_))
        )
    {
        return Err(vm.type_error("Illegal invocation"));
    }
    if let Some(HostData::Document(document)) = super::host_data(vm, &call.this)
        && let Some(value) = super::frames::document_method(vm, document, &call)?
    {
        return Ok(value);
    }
    let name = native_name(&call);
    match name {
        "getElementById" => {
            let wanted = vm.to_text(&call.arg(0))?;
            if vm.page.legacy {
                let node = super::window::legacy_element(vm, &wanted)?;
                return wrap_option(vm, node);
            }
            if wanted.is_empty() {
                return Ok(JsValue::Null);
            }
            let found = search(vm, DOCUMENT, |page, id| {
                page.view(id).is_some_and(|view| {
                    view.attributes
                        .get("id")
                        .is_some_and(|value| *value == wanted)
                })
            });
            wrap_option(vm, found.first().copied())
        }
        "getElementsByName" => {
            let wanted = vm.to_text(&call.arg(0))?;
            let found = search(vm, DOCUMENT, |page, id| {
                page.view(id).is_some_and(|view| {
                    view.attributes
                        .get("name")
                        .is_some_and(|value| *value == wanted)
                })
            });
            node_list(vm, found)
        }
        "getElementsByTagName" => elements_by_tag(vm, DOCUMENT, &call),
        "getElementsByClassName" => elements_by_class(vm, DOCUMENT, &call),
        "querySelector" | "querySelectorAll" => query(vm, DOCUMENT, &call),
        "createElement" => create_element_call(vm, &call, false),
        "createElementNS" => create_element_call(vm, &call, true),
        "createCDATASection" => {
            // The page document refuses CDATA as HTML does; another document (`new Document()`)
            // takes it. ponytail: a CDATA section is kept as a text node, so its nodeType is 3.
            if this_node(vm, &call)? == DOCUMENT {
                return Err(vm.dom_exception(
                    "NotSupportedError",
                    "CDATA sections are not supported in HTML documents.",
                ));
            }
            let text = vm.to_text(&call.arg(0))?;
            let node = vm
                .page
                .create("#text", Some(text))
                .map_err(|error| dom_error(vm, error))?;
            wrap(vm, node)
        }
        "createAttribute" => attr_dom::create_attribute(vm, &call, false),
        "createAttributeNS" => attr_dom::create_attribute(vm, &call, true),
        "createTextNode" | "createComment" => {
            let text = vm.to_text(&call.arg(0))?;
            let tag = if name == "createTextNode" {
                "#text"
            } else {
                "#comment"
            };
            let node = vm
                .page
                .create(tag, Some(text))
                .map_err(|error| dom_error(vm, error))?;
            wrap(vm, node)
        }
        "createDocumentFragment" => {
            let node = vm
                .page
                .create("#document-fragment", None)
                .map_err(|error| dom_error(vm, error))?;
            wrap(vm, node)
        }
        "createProcessingInstruction" => {
            let owner = this_node(vm, &call)?;
            implementation::create_processing_instruction(vm, owner, &call)
        }
        "createEvent" => {
            let interface = vm.to_text(&call.arg(0))?;
            super::events::create_legacy_event(vm, &interface)
        }
        "hasFocus" => Ok(JsValue::Boolean(true)),
        "elementFromPoint" => {
            let hit = element_from_point(vm, &call)?;
            wrap_option(vm, hit)
        }
        "elementsFromPoint" => {
            let hit = element_from_point(vm, &call)?;
            let mut chain = hit.map(|node| vec![node]).unwrap_or_default();
            if let Some(node) = hit {
                chain.extend(vm.page.ancestors(node));
            }
            let mut wrapped = Vec::with_capacity(chain.len());
            for node in chain {
                wrapped.push(wrap(vm, node)?);
            }
            Ok(JsValue::Object(vm.new_array(wrapped)?))
        }
        "getSelection" => super::range::selection(vm),
        "createRange" => super::range::new_range(vm),
        "open" | "close" => Ok(JsValue::Undefined),
        "write" | "writeln" => Err(vm.unsupported_error("document.write")),
        "importNode" | "adoptNode" => {
            let node = arg_node(vm, &call, 0)?;
            if name == "adoptNode" {
                if vm.page.parent(node).is_some() {
                    remove_node(vm, node)?;
                }
                return Ok(call.arg(0));
            }
            let copy = clone_node(vm, node, call.arg(1).truthy())?;
            wrap(vm, copy)
        }
        _ => Ok(JsValue::Undefined),
    }
}

/** document.implementation.createHTMLDocument, ported in frames. */
pub(in crate::v9) fn create_html_document(vm: &mut Vm, call: Call) -> Res<JsValue> {
    super::frames::create_html_document(vm, call)
}

/** classList methods. */
/** Runs the steps below, then the custom element reactions and records they queued. */
pub(in crate::v9) fn class_list(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| class_list_steps(vm, call))
}

/** values, keys, entries and forEach over the tokens of `set`, in its order. */
fn token_iteration(vm: &mut Vm, call: &Call, name: &str, set: &TokenSet) -> Res<JsValue> {
    let tokens: Vec<&str> = set.iter().collect();
    if name == "forEach" {
        let callback = call.arg(0);
        if !vm.is_callable(&callback) {
            return Err(vm.type_error("DOMTokenList.forEach callback is not a function"));
        }
        for (index, token) in tokens.iter().enumerate() {
            let args = vec![
                JsValue::string(token)?,
                JsValue::Number(index as f64),
                call.this.clone(),
            ];
            vm.call(callback.clone(), call.arg(1), args)?;
        }
        return Ok(JsValue::Undefined);
    }
    let mut items = Vec::with_capacity(tokens.len());
    for (index, token) in tokens.iter().enumerate() {
        items.push(match name {
            "keys" => JsValue::Number(index as f64),
            "values" => JsValue::string(token)?,
            _ => {
                let pair = vec![JsValue::Number(index as f64), JsValue::string(token)?];
                JsValue::Object(vm.new_array(pair)?)
            }
        });
    }
    let array = JsValue::Object(vm.new_array(items)?);
    let method = vm.get(&array, &Key::Sym(crate::v9::vm::realm::symbol::ITERATOR))?;
    vm.call(method, array, Vec::new())
}

/** The class_list steps themselves: read the token set, run the method on it, and when the method
mutates the set, run the update steps: write the attribute back if it exists or the set is not empty. */
fn class_list_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some((node, attribute)) = token_list_target(vm, &call.this) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let mut arguments = Vec::new();
    for value in call.args {
        arguments.push(vm.to_text(value)?);
    }
    let name = native_name(&call).to_string();
    let current = vm.page.attribute_value(node, &attribute);
    let present = current.is_some();
    let mut set = token_set_for(vm, node, &attribute, current.as_deref().unwrap_or(""));
    let tag = vm.page.tag(node);
    let supported = token_list::supported_tokens(&tag, &attribute);
    let (result, updated) = token_method(vm, &call, &name, &arguments, &mut set, supported)?;
    if updated && (present || !set.is_empty()) {
        let text = set.serialize();
        vm.page
            .set_attribute(node, &attribute, Some(&text))
            .map_err(|error| dom_error(vm, error))?;
        set.mark_written(text);
    }
    vm.page.token_sets.insert((node, attribute), set);
    Ok(result)
}

/** The node and attribute a token list object stands for, or None for any other value. */
fn token_list_target(vm: &Vm, value: &JsValue) -> Option<(usize, String)> {
    match super::host_data(vm, value)? {
        HostData::ClassList(node) => Some((node, "class".to_string())),
        HostData::TokenList(node, attribute) => Some((node, attribute)),
        _ => None,
    }
}

/** The DOMTokenList an element hands out for an attribute. The same object comes back on every read,
as the DOM standard requires, so `el.classList === el.classList`; the cache is weak, as node wrappers are. */
fn token_list_object(vm: &mut Vm, node: usize, attribute: &str) -> Res<JsValue> {
    if let Some(id) = vm
        .page
        .state
        .token_lists
        .get(&node)
        .and_then(|by_attribute| by_attribute.get(attribute))
        && vm.heap.get(*id).is_some()
    {
        return Ok(JsValue::Object(*id));
    }
    let data = if attribute == "class" {
        HostData::ClassList(node)
    } else {
        HostData::TokenList(node, attribute.to_string())
    };
    let proto = realm().ids.class_list_prototype;
    let id = vm.alloc(Object::new(Class::Host(Box::new(data)), Some(proto)))?;
    vm.page
        .state
        .token_lists
        .entry(node)
        .or_default()
        .insert(attribute.to_string(), id);
    Ok(JsValue::Object(id))
}

/** The `value` setter: assigning a token list's value writes its attribute, as the IDL says. */
pub(in crate::v9) fn class_list_value_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| class_list_value_steps(vm, call))
}

/** The value setter's steps: the attribute takes the text as it is given. */
fn class_list_value_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some((node, attribute)) = token_list_target(vm, &call.this) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let text = vm.to_text(&call.arg(0))?;
    vm.page
        .set_attribute(node, &attribute, Some(&text))
        .map_err(|error| dom_error(vm, error))?;
    Ok(JsValue::Undefined)
}

/** The token set of an attribute: the one this request kept while the attribute still holds the
text it was read from, otherwise one parsed from `current` now. */
fn token_set_for(vm: &mut Vm, node: usize, attribute: &str, current: &str) -> TokenSet {
    match vm.page.token_sets.remove(&(node, attribute.to_string())) {
        Some(set) if set.source() == current => set,
        _ => TokenSet::parse(current),
    }
}

/** Runs one DOMTokenList method over its set. Returns its value and whether it runs the update
steps: add and remove always do, while toggle and replace do only when they change the set. The
arguments a method needs are counted first, and the tokens a mutating method adds or removes are
checked before the set changes. */
fn token_method(
    vm: &mut Vm,
    call: &Call,
    name: &str,
    arguments: &[String],
    set: &mut TokenSet,
    supported: Option<&[&str]>,
) -> Res<(JsValue, bool)> {
    let required = token_list::required_arguments(name);
    if arguments.len() < required {
        let message =
            format!("Failed to execute '{name}' on 'DOMTokenList': {required} argument required.");
        return Err(vm.type_error(&message));
    }
    if token_list::MUTATING_METHODS.contains(&name) {
        let checked = match name {
            "add" | "remove" => arguments,
            _ => &arguments[..required],
        };
        token_list::check_tokens(checked).map_err(|error| name_error(vm, error))?;
    }
    let value = |value: JsValue| Ok((value, false));
    match name {
        "contains" => value(JsValue::Boolean(set.contains(&arguments[0]))),
        "item" => value(item_value(vm, call, set)?),
        "length" => value(JsValue::Number(set.len() as f64)),
        "value" | "toString" => value(JsValue::string(set.source())?),
        // DOMTokenList is iterable over its ordered token set, as in browsers: YouTube spreads
        // and for-of loops classList, and a TypeError there stopped its rendering.
        "values" | "keys" | "entries" | "forEach" => value(token_iteration(vm, call, name, set)?),
        "supports" => match supported {
            Some(list) => value(JsValue::Boolean(token_list::is_supported(
                list,
                &arguments[0],
            ))),
            None => Err(vm.type_error("DOMTokenList has no supported tokens.")),
        },
        "add" => {
            for token in arguments {
                set.insert(token);
            }
            Ok((JsValue::Undefined, true))
        }
        "remove" => {
            for token in arguments {
                set.remove(token);
            }
            Ok((JsValue::Undefined, true))
        }
        "toggle" => toggle(call, set, &arguments[0]),
        "replace" => {
            let found = set.replace(&arguments[0], &arguments[1]);
            Ok((JsValue::Boolean(found), found))
        }
        _ => value(JsValue::Undefined),
    }
}

/** toggle(token, force): a force that is given decides the outcome; otherwise the token flips.
Returns the outcome and whether the set changed, since only a change runs the update steps. */
fn toggle(call: &Call, set: &mut TokenSet, token: &str) -> Res<(JsValue, bool)> {
    let forced = !matches!(call.arg(1), JsValue::Undefined);
    let wanted = if forced {
        call.arg(1).truthy()
    } else {
        !set.contains(token)
    };
    let changed = if wanted {
        set.insert(token)
    } else {
        set.remove(token)
    };
    Ok((JsValue::Boolean(wanted), changed))
}

/** DOMTokenList.item(index): the token at that index, or null past the end or below zero. */
fn item_value(vm: &mut Vm, call: &Call, set: &TokenSet) -> Res<JsValue> {
    let index = vm.to_integer(&call.arg(0))?;
    let found = (index >= 0.0 && (index as usize) < set.len())
        .then(|| set.nth(index as usize))
        .flatten();
    match found {
        Some(token) => JsValue::string(token).map_err(Into::into),
        None => Ok(JsValue::Null),
    }
}

/** style object methods and cssText. */
/** Runs the steps below, then the custom element reactions and records they queued. */
pub(in crate::v9) fn style_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| style_method_steps(vm, call))
}

/** The style_method steps themselves. */
fn style_method_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (node, computed) = match super::host_data(vm, &call.this) {
        Some(HostData::Style(node)) => (node, false),
        Some(HostData::Computed(node)) => (node, true),
        _ => return Err(vm.type_error("Illegal invocation")),
    };
    let name = native_name(&call);
    if computed && matches!(name, "setProperty" | "removeProperty") {
        return Err(vm.throw(
            crate::v9::vm::error::ErrorKind::Error,
            "NoModificationAllowedError: computed styles are read-only.",
        ));
    }
    match name {
        "getPropertyValue" if computed => {
            let property = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
            let value = computed_value(vm, node, &property);
            JsValue::string(&value).map_err(Into::into)
        }
        "getPropertyValue" => {
            let property = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
            let value = style_map(vm, node)
                .into_iter()
                .find(|(name, _)| *name == property)
                .map(|(_, value)| value)
                .unwrap_or_default();
            JsValue::string(&value).map_err(Into::into)
        }
        "setProperty" => {
            let property = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
            let value = if call.arg(1).nullish() {
                String::new()
            } else {
                vm.to_text(&call.arg(1))?
            };
            set_style(vm, node, &property, &value)?;
            Ok(JsValue::Undefined)
        }
        "removeProperty" => {
            let property = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
            set_style(vm, node, &property, "")?;
            Ok(JsValue::Undefined)
        }
        "getPropertyPriority" => JsValue::string("").map_err(Into::into),
        // The declared property names, in order: `style.length`, `style.item(i)` and
        // `[...style]`, as a CSSStyleDeclaration is indexed and iterable in browsers.
        "length" | "item" | "values" if !computed => {
            let names: Vec<String> = style_map(vm, node)
                .into_iter()
                .map(|(name, _)| name)
                .collect();
            match name {
                "length" => Ok(JsValue::Number(names.len() as f64)),
                "item" => {
                    let index = vm.to_integer(&call.arg(0))?;
                    let found = (index >= 0.0).then(|| names.get(index as usize)).flatten();
                    JsValue::string(found.map_or("", String::as_str)).map_err(Into::into)
                }
                _ => {
                    let items = names
                        .iter()
                        .map(|name| JsValue::string(name))
                        .collect::<Result<Vec<_>, _>>()?;
                    let array = JsValue::Object(vm.new_array(items)?);
                    let method =
                        vm.get(&array, &Key::Sym(crate::v9::vm::realm::symbol::ITERATOR))?;
                    vm.call(method, array, Vec::new())
                }
            }
        }
        "cssText" => {
            let text = vm
                .page
                .view(node)
                .and_then(|view| view.attributes.get("style").cloned())
                .unwrap_or_default();
            JsValue::string(&text).map_err(Into::into)
        }
        "length" => Ok(JsValue::Number(style_map(vm, node).len() as f64)),
        _ => Ok(JsValue::Undefined),
    }
}

/** style.cssText setter. */
/** Runs the steps below, then the custom element reactions and records they queued. */
pub(in crate::v9) fn style_text_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| style_text_set_steps(vm, call))
}

/** The style_text_set steps themselves. */
fn style_text_set_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(HostData::Style(node)) = super::host_data(vm, &call.this) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let text = vm.to_text(&call.arg(0))?;
    let value = if text.trim().is_empty() {
        None
    } else {
        Some(text.as_str())
    };
    vm.page
        .set_attribute(node, "style", value)
        .map_err(|error| dom_error(vm, error))?;
    Ok(JsValue::Undefined)
}

/** NodeList methods: item, forEach, entries, keys, values. */
pub(in crate::v9) fn list_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(data) = super::host_data(vm, &call.this) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    if native_name(&call) == "item" {
        return list_item(vm, &call, &data);
    }
    let nodes = list_nodes(vm, &data);
    match native_name(&call) {
        "namedItem" => {
            let key = vm.to_text(&call.arg(0))?;
            super::form_submit::named(vm, &nodes, &key)
        }
        "forEach" => {
            let callback = call.arg(0);
            for (index, node) in nodes.into_iter().enumerate() {
                let element = wrap(vm, node)?;
                vm.call(
                    callback.clone(),
                    call.arg(1),
                    vec![element, JsValue::Number(index as f64), call.this.clone()],
                )?;
            }
            Ok(JsValue::Undefined)
        }
        _ => {
            let mut values = Vec::new();
            for node in nodes {
                values.push(wrap(vm, node)?);
            }
            let array = vm.new_array(values)?;
            let method = vm.get(&JsValue::Object(array), &Key::from(native_name(&call)))?;
            vm.call(method, JsValue::Object(array), Vec::new())
        }
    }
}

/** Constructors scripts may not call directly but need for instanceof. */
pub(in crate::v9) fn illegal_constructor(vm: &mut Vm, _: Call) -> Res<JsValue> {
    Err(vm.type_error("Illegal constructor"))
}
