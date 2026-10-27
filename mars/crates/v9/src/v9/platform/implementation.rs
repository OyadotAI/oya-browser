/**
 * DOMImplementation and the nodes it and the Document constructor make: `document.implementation`
 * for the page and for each script-made document, createDocumentType, createDocument, `new
 * Document()`, and Document.createProcessingInstruction. Every node made here records the
 * document it belongs to, so ownerDocument answers with the document the call came from.
 */
use super::dom::{self, DOCUMENT, node_of, wrap};
use super::frames::{self, DOCTYPE, DOCUMENT_NODE, SVG_TYPE, XHTML_TYPE, XML_TYPE};
use super::namespaces::{self, ElementNs, HTML_NAMESPACE, NameKind, QName, SVG_NAMESPACE};
use crate::v9::value::JsValue;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::{Builder, realm};
use crate::v9::vm::{Call, Res, Thrown, Vm};
use std::sync::OnceLock;

/** The tag of a processing instruction node; its target is an attribute and its data the text. */
pub(super) const PROCESSING_INSTRUCTION: &str = "#processing-instruction";
/** The attribute a processing instruction keeps its target under. */
const TARGET: &str = "target";
/** The DOM error name for a name or data the spec rejects. */
const INVALID_CHARACTER: &str = "InvalidCharacterError";
/** The character a doctype or document name may not contain (with whitespace). */
const NAME_TERMINATOR: char = '>';
/** The sequence a processing instruction's data may not contain. */
const PI_CLOSE: &str = "?>";
/** Required arguments of createDocument: the namespace and qualified name. */
const CREATE_DOCUMENT_ARGS: usize = 2;
/** Required arguments of createDocumentType: the name, public id and system id. */
const CREATE_DOCUMENT_TYPE_ARGS: usize = 3;
/** Required arguments of createProcessingInstruction: the target and data. */
const PI_ARGS: usize = 2;
/** The length createDocument reports: its optional doctype is not counted. */
const CREATE_DOCUMENT_LENGTH: u32 = 2;
/** The length createDocumentType reports. */
const CREATE_DOCUMENT_TYPE_LENGTH: u32 = 3;
/** The length createProcessingInstruction reports. */
pub(super) const PI_LENGTH: u32 = 2;
/** XML 1.0 (fifth edition) NameStartChar code point ranges, inclusive. */
const NAME_START: [(u32, u32); 16] = [
    (0x3A, 0x3A),
    (0x41, 0x5A),
    (0x5F, 0x5F),
    (0x61, 0x7A),
    (0xC0, 0xD6),
    (0xD8, 0xF6),
    (0xF8, 0x2FF),
    (0x370, 0x37D),
    (0x37F, 0x1FFF),
    (0x200C, 0x200D),
    (0x2070, 0x218F),
    (0x2C00, 0x2FEF),
    (0x3001, 0xD7FF),
    (0xF900, 0xFDCF),
    (0xFDF0, 0xFFFD),
    (0x10000, 0xEFFFF),
];
/** The characters a name may add after its first (NameChar beyond NameStartChar). */
const NAME_EXTRA: [(u32, u32); 5] = [
    (0x2D, 0x2E),
    (0x30, 0x39),
    (0xB7, 0xB7),
    (0x300, 0x36F),
    (0x203F, 0x2040),
];

/** ProcessingInstruction.prototype, set once while the realm is built. */
static PI_PROTOTYPE: OnceLock<u32> = OnceLock::new();

/** Build DOMImplementation's prototype and the page's own implementation object, which
`document.implementation` returns for the page; the page's and every script-made document's
implementation share this prototype. */
pub(super) fn install_prototype(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.method(
        proto,
        "createDocumentType",
        CREATE_DOCUMENT_TYPE_LENGTH,
        create_document_type,
    );
    b.method(
        proto,
        "createDocument",
        CREATE_DOCUMENT_LENGTH,
        create_document,
    );
    b.method(proto, "createHTMLDocument", 1, dom::create_html_document);
    b.method(proto, "hasFeature", 0, super::install::yes);
    b.ids.implementation_prototype = proto;
    let page = b.object(Some(proto));
    b.ids.implementation = page;
}

/** Install ProcessingInstruction and the DOMImplementation and Document constructors that
scripts reach through instanceof; neither constructs anything from script (Document does). */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.text_prototype));
    b.getter(proto, "target", target_get);
    let ctor = b.constructor(
        "ProcessingInstruction",
        0,
        dom::illegal_constructor,
        proto,
        None,
    );
    b.global("ProcessingInstruction", ctor);
    let _ = PI_PROTOTYPE.set(proto);
    let implementation = b.ids.implementation_prototype;
    let ctor = b.constructor(
        "DOMImplementation",
        0,
        dom::illegal_constructor,
        implementation,
        None,
    );
    b.global("DOMImplementation", ctor);
}

/** The prototype a processing instruction node's wrapper gets. */
pub(super) fn processing_instruction_prototype() -> Option<u32> {
    let _ = realm();
    PI_PROTOTYPE.get().copied()
}

/** `document.implementation`: the page's object for the page, and for a script-made document its
own, made on first ask and kept so the identity holds. */
pub(super) fn implementation_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    match node_of(vm, &call.this) {
        Some(document) if document != DOCUMENT => frame_implementation(vm, document),
        _ => Ok(JsValue::Object(realm().ids.implementation)),
    }
}

/** The implementation object a script-made document owns, made on first ask. */
fn frame_implementation(vm: &mut Vm, document: usize) -> Res<JsValue> {
    let kept = vm.page.state.implementation_of.get(&document).copied();
    if let Some(id) = kept
        && vm.heap.get(id).is_some()
    {
        return Ok(JsValue::Object(id));
    }
    let proto = realm().ids.implementation_prototype;
    let id = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    vm.page.state.implementation_of.insert(document, id);
    vm.page.state.implementations.insert(id, document);
    Ok(JsValue::Object(id))
}

/** The document an implementation method's `this` belongs to: the page for the page's object,
the owning document for a script-made one. Anything else is an illegal invocation. */
fn owner_of(vm: &mut Vm, call: &Call) -> Res<usize> {
    let Some(id) = call.this.object() else {
        return Err(vm.type_error("Illegal invocation"));
    };
    if id == realm().ids.implementation {
        return Ok(DOCUMENT);
    }
    let owner = vm.page.state.implementations.get(&id).copied();
    match owner {
        Some(owner) => Ok(owner),
        None => Err(vm.type_error("Illegal invocation")),
    }
}

/** A WebIDL required-argument check: fewer arguments than the signature needs is a TypeError. */
fn require_args(vm: &mut Vm, call: &Call, count: usize) -> Res<()> {
    if call.args.len() < count {
        return Err(vm.type_error("Not enough arguments"));
    }
    Ok(())
}

/** The DOM's InvalidCharacterError, naming the rejected text. */
fn invalid_character(vm: &mut Vm, name: &str) -> Thrown {
    let message =
        format!("Failed to execute on 'DOMImplementation': '{name}' is not a valid name.");
    vm.dom_exception(INVALID_CHARACTER, &message)
}

/** Whether a code point is in one of the inclusive ranges. */
fn in_ranges(code: u32, ranges: &[(u32, u32)]) -> bool {
    ranges
        .iter()
        .any(|(low, high)| (*low..=*high).contains(&code))
}

/** Whether a name matches the XML Name production, which a processing instruction target must. */
pub(super) fn is_xml_name(name: &str) -> bool {
    let mut chars = name.chars().map(u32::from);
    let Some(first) = chars.next() else {
        return false;
    };
    in_ranges(first, &NAME_START)
        && chars.all(|code| in_ranges(code, &NAME_START) || in_ranges(code, &NAME_EXTRA))
}

/** The looser rule createDocumentType and createDocument apply, which Blink's behaviour pins:
a name may hold anything but whitespace and '>', so "1foo" and "@foo" are accepted. */
fn lax_name(name: &str) -> bool {
    !name
        .chars()
        .any(|c| c.is_ascii_whitespace() || c == NAME_TERMINATOR)
}

/** Where a doctype goes for createDocument: null or undefined for none, a DocumentType otherwise. */
fn optional_doctype(vm: &mut Vm, value: &JsValue) -> Res<Option<usize>> {
    if value.nullish() {
        return Ok(None);
    }
    match node_of(vm, value) {
        Some(node) if vm.page.tag(node) == DOCTYPE => Ok(Some(node)),
        _ => Err(vm.type_error(
            "Failed to execute 'createDocument' on 'DOMImplementation': parameter 3 is not of type 'DocumentType'.",
        )),
    }
}

/** DOMImplementation.createDocumentType(name, publicId, systemId): a doctype, owned by the
implementation's document and not yet in any tree. */
fn create_document_type(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let owner = owner_of(vm, &call)?;
    require_args(vm, &call, CREATE_DOCUMENT_TYPE_ARGS)?;
    let name = vm.to_text(&call.arg(0))?;
    let public = vm.to_text(&call.arg(1))?;
    let system = vm.to_text(&call.arg(2))?;
    if !lax_name(&name) {
        return Err(invalid_character(vm, &name));
    }
    let node = frames::doctype(vm, &name, &public, &system)?;
    vm.page.state.owners.insert(node, owner);
    wrap(vm, node)
}

/** Move a doctype into a document as its doctype. A doctype in a tree is not moved (the page's
own is never taken), and a detached one leaves its old parent. */
fn attach_doctype(vm: &mut Vm, document: usize, doctype: usize) -> Res<()> {
    if !vm.page.is_projected(doctype) {
        vm.page
            .remove(doctype)
            .map_err(|error| dom::dom_error(vm, error))?;
    }
    frames::append(vm, document, doctype)?;
    vm.page.state.owners.insert(doctype, document);
    Ok(())
}

/** The media type createDocument gives a document, from its namespace: HTML is XHTML, SVG is SVG,
and anything else (including the null namespace) is plain XML, as the DOM standard says. */
fn create_document_type_for(namespace: Option<&str>) -> &'static str {
    match namespace {
        Some(HTML_NAMESPACE) => XHTML_TYPE,
        Some(SVG_NAMESPACE) => SVG_TYPE,
        _ => XML_TYPE,
    }
}

/** createDocument's qualifiedName: null names no root element (WebIDL's nullable argument), while
undefined is the text "undefined" like any other value. */
fn qualified_name_arg(vm: &mut Vm, value: &JsValue) -> Res<String> {
    if matches!(value, JsValue::Null) {
        return Ok(String::new());
    }
    vm.to_text(value)
}

/** The root element name createDocument makes, validated for `namespace`. An empty qualified name
makes no root; any other must pass the DOM's name and prefix rules, or the call throws. */
fn root_name(vm: &mut Vm, namespace: Option<&str>, qualified: &str) -> Res<Option<QName>> {
    if qualified.is_empty() {
        return Ok(None);
    }
    namespaces::validate_and_extract(namespace, qualified, NameKind::Element)
        .map(Some)
        .map_err(|error| dom::name_error(vm, error))
}

/** Put the optional doctype and the root element into a new document, the root carrying its
namespace so namespaceURI and localName answer as the DOM does. */
fn populate(
    vm: &mut Vm,
    document: usize,
    doctype: Option<usize>,
    root: Option<(&str, Option<String>, QName)>,
) -> Res<()> {
    if let Some(doctype) = doctype {
        attach_doctype(vm, document, doctype)?;
    }
    if let Some((qualified, namespace, qname)) = root {
        let node = frames::create(vm, qualified, None)?;
        let record = ElementNs {
            namespace,
            prefix: qname.prefix,
            local: qname.local,
        };
        vm.page.set_element_namespace(node, record);
        frames::append(vm, document, node)?;
    }
    Ok(())
}

/** DOMImplementation.createDocument(namespace, qualifiedName, doctype): an XML document with the
optional doctype and, for a non-empty qualified name, a root element in `namespace`. The
document's media type and prototype (XMLDocument) follow the namespace, as the DOM says. */
fn create_document(vm: &mut Vm, call: Call) -> Res<JsValue> {
    owner_of(vm, &call)?;
    require_args(vm, &call, CREATE_DOCUMENT_ARGS)?;
    let namespace = dom::optional_string_arg(vm, &call, 0)?;
    let qualified = qualified_name_arg(vm, &call.arg(1))?;
    let doctype = optional_doctype(vm, &call.arg(2))?;
    let root = root_name(vm, namespace.as_deref(), &qualified)?;
    let media_type = create_document_type_for(namespace.as_deref());
    let document = frames::create(vm, DOCUMENT_NODE, None)?;
    let root = root.map(|qname| (qualified.as_str(), namespace, qname));
    populate(vm, document, doctype, root)?;
    frames::set_content_type(vm, document, media_type);
    frames::wrap_document_as(vm, document, realm().ids.xml_document_prototype)
}

/** `new Document()`: an empty document, with no doctype, root element or children. */
pub(super) fn construct_document(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("Failed to construct 'Document': Please use the 'new' operator."));
    }
    let node = frames::create(vm, DOCUMENT_NODE, None)?;
    frames::set_content_type(vm, node, XML_TYPE);
    frames::wrap_document(vm, node)
}

/** Document.createProcessingInstruction(target, data), made for `owner`. The target must be an
XML name and the data may not contain "?>", as the DOM requires. */
pub(super) fn create_processing_instruction(
    vm: &mut Vm,
    owner: usize,
    call: &Call,
) -> Res<JsValue> {
    require_args(vm, call, PI_ARGS)?;
    let target = vm.to_text(&call.arg(0))?;
    let data = vm.to_text(&call.arg(1))?;
    if !is_xml_name(&target) || data.contains(PI_CLOSE) {
        return Err(invalid_character(vm, &target));
    }
    let node = frames::create(vm, PROCESSING_INSTRUCTION, Some(data))?;
    if let Some(record) = vm.page.state.detached.get_mut(&node) {
        record.attributes.insert(TARGET.to_string(), target);
    }
    vm.page.state.owners.insert(node, owner);
    wrap(vm, node)
}

/** ProcessingInstruction.target: the target the node was made with. */
fn target_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    let target = vm
        .page
        .view(node)
        .and_then(|view| view.attributes.get(TARGET).cloned())
        .unwrap_or_default();
    JsValue::string(&target).map_err(Into::into)
}
