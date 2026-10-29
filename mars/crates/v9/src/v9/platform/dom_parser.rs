/**
 * DOMParser: `parseFromString` builds a document from markup, as the DOM Parsing standard does,
 * and returns it. text/html goes through the HTML tree builder, so the document has html, head
 * and body; text/xml, application/xml, application/xhtml+xml and image/svg+xml go through the
 * namespace-aware XML reader, so the document's root is the first element of the source. A
 * source that is not well-formed becomes the parsererror document. The document is a real
 * Document (or XMLDocument) with its own contentType and the page's URL, and its location is
 * null, since it has no browsing context. Its nodes adopt into the page when appended.
 */
use super::frames::{self, DOCUMENT_NODE, HTML_TYPE, SVG_TYPE, XHTML_TYPE, XML_TYPE};
use super::xml_frames;
use crate::v9::value::JsValue;
use crate::v9::vm::realm::{Builder, realm};
use crate::v9::vm::{Call, Res, Thrown, Vm};

/** The XML media type DOMParser accepts beside application/xml, which it also parses as XML. */
const TEXT_XML_TYPE: &str = "text/xml";
/** The media types parseFromString accepts: the DOMParserSupportedType enumeration. */
const SUPPORTED_TYPES: [&str; 5] = [HTML_TYPE, TEXT_XML_TYPE, XML_TYPE, XHTML_TYPE, SVG_TYPE];
/** The arguments parseFromString requires: the markup and the media type. */
const PARSE_ARGS: usize = 2;
/** The TypeError message for a call with fewer arguments than parseFromString requires. */
const MISSING_ARGUMENTS: &str =
    "Failed to execute 'parseFromString' on 'DOMParser': 2 arguments required";

/** Install the DOMParser constructor and its prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.host_method(proto, "parseFromString", 2, parse_from_string);
    let ctor = b.constructor("DOMParser", 0, construct, proto, None);
    b.global("DOMParser", ctor);
}

/** `new DOMParser()`: a plain object over the prototype; it holds no state. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let fallback = crate::v9::vm::realm::realm().ids.object_prototype;
    let proto = match &call.new_target {
        Some(target) => vm.prototype_for(target, fallback)?,
        None => return Err(vm.type_error("Constructor DOMParser requires 'new'")),
    };
    let id = vm.alloc(crate::v9::vm::heap::Object::new(
        crate::v9::vm::data::Class::Plain,
        Some(proto),
    ))?;
    Ok(JsValue::Object(id))
}

/** `parseFromString(markup, type)`: a new document holding the parsed markup. The type must be
one of the supported media types, and both arguments are required, as WebIDL demands. */
fn parse_from_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.args.len() < PARSE_ARGS {
        return Err(vm.type_error(MISSING_ARGUMENTS));
    }
    let markup = vm.to_text(&call.arg(0))?;
    let media = vm.to_text(&call.arg(1))?;
    match supported_type(&media) {
        Some(media_type) => new_document(vm, &markup, media_type),
        None => Err(unsupported_type(vm, &media)),
    }
}

/** The TypeError for a media type outside the supported list, naming the value that was given. */
fn unsupported_type(vm: &mut Vm, media: &str) -> Thrown {
    let message = format!(
        "Failed to execute 'parseFromString' on 'DOMParser': The provided value '{media}' is not a valid enum value of type DOMParserSupportedType."
    );
    vm.type_error(&message)
}

/** The supported media type `media` names exactly, or None for any other text. */
fn supported_type(media: &str) -> Option<&'static str> {
    SUPPORTED_TYPES
        .iter()
        .copied()
        .find(|known| *known == media)
}

/** A new document of `media_type` holding `markup`, wrapped as Document for HTML and as
XMLDocument for every XML type. Its URL is the page's, since DOMParser takes it from the page. */
fn new_document(vm: &mut Vm, markup: &str, media_type: &str) -> Res<JsValue> {
    let document = frames::create(vm, DOCUMENT_NODE, None)?;
    xml_frames::build_source(vm, document, media_type, markup)?;
    vm.page.state.parsed_documents.insert(document);
    let ids = &realm().ids;
    let proto = if media_type == HTML_TYPE {
        ids.document_prototype
    } else {
        ids.xml_document_prototype
    };
    frames::wrap_document_as(vm, document, proto)
}
