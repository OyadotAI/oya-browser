use super::FrameRequest;
/**
 * Documents beyond the page's own: the same-origin document of an `about:blank` or `srcdoc`
 * iframe (Blink's HTMLFrameOwnerElement::contentDocument and contentWindow), a script-made one
 * from `document.implementation.createHTMLDocument` (DOMImplementation), and `open`, `write`
 * and `close` on them (Document::open, Document::write, Document::close).
 *
 * Each document is a detached `#document` node holding an optional `#doctype` and its html
 * element, in the same node store as script-created nodes. An iframe with any other `src` has
 * no document a script may reach, so its contentDocument and contentWindow stay null: Mars
 * loads no nested browsing contexts, and cross-origin isolation must hold.
 *
 * Written markup is buffered and parsed when the document closes, so a write split into single
 * characters parses as one. The tree builder places the parsed nodes as the HTML parser's
 * initial, before-html, in-head, after-head and in-body insertion modes do; content inside the
 * body comes from V9's own reader, which has no adoption agency or foster parenting.
 */
use super::dom::{self, dom_error, native_name, wrap, wrap_option};
use super::html::{Parsed, parse_tree};
use super::xml_frames;
use crate::protocol::EngineResult;
use crate::v9::value::JsValue;
use crate::v9::vm::data::{Class, HostData};
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::{Builder, realm};
use crate::v9::vm::{Call, GLOBAL, Res, Vm};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/** The tag of a document node. */
pub(super) const DOCUMENT_NODE: &str = "#document";
/** The tag of a doctype node; its name and ids are attributes. */
pub(super) const DOCTYPE: &str = "#doctype";
/** Iframes scripts connected whose documents V9 has asked the app for and not heard back on. */
const FRAME_REQUESTS_PENDING: usize = 64;
/** Elements the in-head insertion mode keeps in the head. */
const HEAD_CONTENT: [&str; 11] = [
    "base", "basefont", "bgsound", "link", "meta", "noframes", "script", "style", "template",
    "title", "noscript",
];

/** DocumentType.prototype, set once while the realm is built. */
static DOCTYPE_PROTOTYPE: OnceLock<u32> = OnceLock::new();

/** Install DocumentType: its prototype and the name, publicId and systemId getters. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.node_prototype));
    for name in ["name", "publicId", "systemId"] {
        b.getter(proto, name, doctype_get);
    }
    // A doctype is a ChildNode, so it takes the same before, after, replaceWith and remove.
    for name in ["before", "after", "replaceWith", "remove"] {
        b.host_method(proto, name, 0, dom::character_data_method);
    }
    let ctor = b.constructor("DocumentType", 0, dom::illegal_constructor, proto, None);
    b.global("DocumentType", ctor);
    let _ = DOCTYPE_PROTOTYPE.set(proto);
    // CharacterData and the ChildNode mixin on text and comment nodes.
    let text = b.ids.text_prototype;
    for name in [
        "substringData",
        "appendData",
        "insertData",
        "deleteData",
        "replaceData",
        "splitText",
        "before",
        "after",
        "replaceWith",
        "remove",
    ] {
        b.host_method(text, name, 0, dom::character_data_method);
    }
}

/** The prototype a doctype node's wrapper gets. */
pub(super) fn doctype_prototype() -> Option<u32> {
    let _ = realm();
    DOCTYPE_PROTOTYPE.get().copied()
}

/** DocumentType members, read from the node's attributes. */
fn doctype_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    let value = vm
        .page
        .view(node)
        .and_then(|view| view.attributes.get(native_name(&call)).cloned())
        .unwrap_or_default();
    JsValue::string(&value).map_err(Into::into)
}

/** The wrapper of a document node: Document.prototype over `HostData::Document`. */
pub(super) fn wrap_document(vm: &mut Vm, node: usize) -> Res<JsValue> {
    wrap_document_as(vm, node, realm().ids.document_prototype)
}

/** A document's wrapper object over `proto`: Document for HTML documents, XMLDocument for the
documents createDocument makes. A node keeps the wrapper it was first given. */
pub(super) fn wrap_document_as(vm: &mut Vm, node: usize, proto: u32) -> Res<JsValue> {
    if let Some(id) = vm.page.state.wrappers.get(&node)
        && vm.heap.get(*id).is_some()
    {
        return Ok(JsValue::Object(*id));
    }
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::Document(node))),
        Some(proto),
    ))?;
    vm.page.state.wrappers.insert(node, id);
    Ok(JsValue::Object(id))
}

/** The media type a script-made document reports: the one its creator recorded, else text/html. */
pub(super) fn content_type_of(vm: &Vm, document: usize) -> String {
    vm.page
        .state
        .content_types
        .get(&document)
        .map_or_else(|| HTML_TYPE.to_string(), String::clone)
}

/** Record the media type of a script-made document that is not HTML (XML documents). */
pub(super) fn set_content_type(vm: &mut Vm, document: usize, media_type: &str) {
    vm.page
        .state
        .content_types
        .insert(document, media_type.to_string());
}

/** `document.location` of a script-made or frame document: null, since it has no browsing
context of its own. The page's document keeps the window's location. */
pub(super) fn location_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    match super::host_data(vm, &call.this) {
        Some(HostData::Document(_)) => Ok(JsValue::Null),
        _ => super::window::window_get(vm, call),
    }
}

/** A new detached node of `tag`, as a script error on failure. */
pub(super) fn create(vm: &mut Vm, tag: &str, text: Option<String>) -> Res<usize> {
    vm.page
        .create(tag, text)
        .map_err(|error| dom_error(vm, error))
}

/** Append `child` to `parent` (a template's content for a template). */
pub(super) fn append(vm: &mut Vm, parent: usize, child: usize) -> Res<()> {
    let parent = vm
        .page
        .content_or_self(parent)
        .map_err(|error| dom_error(vm, error))?;
    vm.page
        .insert(parent, child, None)
        .map(|_| ())
        .map_err(|error| dom_error(vm, error))
}

/** A doctype node with this name and ids. */
pub(super) fn doctype(vm: &mut Vm, name: &str, public: &str, system: &str) -> Res<usize> {
    let node = create(vm, DOCTYPE, None)?;
    if let Some(record) = vm.page.state.detached.get_mut(&node) {
        record.attributes = BTreeMap::from([
            ("name".to_string(), name.to_string()),
            ("publicId".to_string(), public.to_string()),
            ("systemId".to_string(), system.to_string()),
        ]);
    }
    Ok(node)
}

/** A document node holding html, head and body, as the initial about:blank document has. */
fn blank_document(vm: &mut Vm) -> Res<(usize, usize, usize)> {
    let document = create(vm, DOCUMENT_NODE, None)?;
    let html = create(vm, "html", None)?;
    let head = create(vm, "head", None)?;
    let body = create(vm, "body", None)?;
    append(vm, document, html)?;
    append(vm, html, head)?;
    append(vm, html, body)?;
    Ok((document, head, body))
}

/** DOMImplementation::createHTMLDocument: a doctype `html`, then html with a head (holding a
title when one is given) and an empty body. */
pub(super) fn create_html_document(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (document, head, _) = blank_document(vm)?;
    let html = vm.page.children(document)[0];
    let doctype = doctype(vm, "html", "", "")?;
    vm.page
        .insert(document, doctype, Some(html))
        .map_err(|error| dom_error(vm, error))?;
    if !matches!(call.arg(0), JsValue::Undefined) {
        let text = vm.to_text(&call.arg(0))?;
        let title = create(vm, "title", None)?;
        let content = create(vm, "#text", Some(text))?;
        append(vm, title, content)?;
        append(vm, head, title)?;
    }
    wrap_document(vm, document)
}

/** The document an iframe shows scripts, made on first ask: only an `about:blank` (or
src-less) or `srcdoc` frame has one; anything else is a browsing context Mars does not load. */
fn frame_document(vm: &mut Vm, frame: usize) -> Res<Option<usize>> {
    if let Some(document) = vm.page.state.frames.get(&frame) {
        return Ok(Some(*document));
    }
    if !vm.page.connected(frame) {
        return Ok(None);
    }
    let view = vm.page.view(frame);
    let attribute = |name: &str| view.as_ref().and_then(|v| v.attributes.get(name).cloned());
    let srcdoc = attribute("srcdoc");
    let blank = attribute("src").is_none_or(|src| {
        let src = src.trim();
        src.is_empty() || src.eq_ignore_ascii_case("about:blank")
    });
    if srcdoc.is_none() && !blank {
        let src = attribute("src").unwrap_or_default();
        return source_document(vm, frame, &src);
    }
    let document = match srcdoc {
        Some(markup) => {
            let document = create(vm, DOCUMENT_NODE, None)?;
            build(vm, document, &markup)?;
            document
        }
        None => blank_document(vm)?.0,
    };
    remember_frame_document(vm, frame, document);
    Ok(Some(document))
}

/** Records that `document` is the document `frame` shows, both ways, so the frame's contentDocument
and a frame document's defaultView each find the other in O(log n). */
fn remember_frame_document(vm: &mut Vm, frame: usize, document: usize) {
    vm.page.state.frames.insert(frame, document);
    vm.page.state.document_frames.insert(document, frame);
}

/** The document of a frame whose `src` the app answered: built from the source it fetched, as its
media type says, or None when the app refused the frame or has not answered. */
fn source_document(vm: &mut Vm, frame: usize, src: &str) -> Res<Option<usize>> {
    let Some(address) = super::net::resolve(vm, src) else {
        return Ok(None);
    };
    let Some(loaded) = vm.page.environment.frame_documents.get(&address).cloned() else {
        return Ok(None);
    };
    let Some(source) = loaded.source else {
        return Ok(None);
    };
    let document = create(vm, DOCUMENT_NODE, None)?;
    xml_frames::build_source(vm, document, &loaded.media_type, &source)?;
    remember_frame_document(vm, frame, document);
    Ok(Some(document))
}

/** An iframe a script connected asks the app for its document, once, as a browser loads a frame
when it is inserted. A frame with no source, an `about:blank` source or a `srcdoc` loads nothing
of its own. */
pub(in crate::v9) fn connected(vm: &mut Vm, node: usize) -> Res<()> {
    let Some(src) = vm.page.attribute_value(node, "src") else {
        return Ok(());
    };
    let src = src.trim();
    let inert = src.is_empty()
        || src.eq_ignore_ascii_case("about:blank")
        || vm.page.attribute_value(node, "srcdoc").is_some();
    let pending = vm.page.state.frame_loads.contains_key(&node)
        || vm.page.state.frame_loads.len() >= FRAME_REQUESTS_PENDING;
    if inert || pending {
        return Ok(());
    }
    let Some(url) = super::net::resolve(vm, src) else {
        return Ok(());
    };
    vm.page.state.frame_loads.insert(node, url.clone());
    vm.page.state.frame_requests.push(FrameRequest { url });
    Ok(())
}

/** Fire `load` on each frame whose answer has arrived, with a document or a refusal. A frame that
left the document meanwhile fires nothing, as a removed frame does not load. */
pub(in crate::v9) fn run_frames(vm: &mut Vm) -> EngineResult<()> {
    let answered: Vec<usize> = vm
        .page
        .state
        .frame_loads
        .iter()
        .filter(|(_, url)| vm.page.environment.frame_documents.contains_key(*url))
        .map(|(element, _)| *element)
        .collect();
    for element in answered {
        vm.page.state.frame_loads.remove(&element);
        if vm.page.connected(element) {
            fire_load(vm, element)?;
        }
    }
    Ok(())
}

/** Fire one frame's `load`; an uncaught error in its handler goes to the console. */
fn fire_load(vm: &mut Vm, element: usize) -> EngineResult<()> {
    let fired =
        super::fire(vm, super::Target::Node(element), "load", false).and_then(|_| vm.drain_jobs());
    match fired {
        Ok(()) => Ok(()),
        Err(crate::v9::vm::Thrown::Value(error)) => {
            vm.report_error(&error);
            Ok(())
        }
        Err(crate::v9::vm::Thrown::Fatal(error)) => Err(error),
    }
}

/** A frame element the document re-identified keeps its pending answer. */
pub(in crate::v9) fn adopt(page: &mut super::Page, old: usize, new: usize) {
    if let Some(url) = page.state.frame_loads.remove(&old) {
        page.state.frame_loads.insert(new, url);
    }
}

/** HTMLIFrameElement's contentDocument and contentWindow. */
pub(super) fn frame_get(vm: &mut Vm, frame: usize, name: &str) -> Res<JsValue> {
    let Some(document) = frame_document(vm, frame)? else {
        // A frame showing another document's own browsing context answers with its proxy.
        if name == "contentWindow" && browsing_context(vm, frame) {
            return super::messaging::proxy(vm, frame as i64);
        }
        return Ok(JsValue::Null);
    };
    if name == "contentDocument" {
        return wrap_document(vm, document);
    }
    content_window(vm, frame, document)
}

/** Whether the iframe loads a document of its own from `src`, which the app runs apart. */
fn browsing_context(vm: &mut Vm, frame: usize) -> bool {
    vm.page.tag(frame) == "iframe"
        && vm.page.connected(frame)
        && vm.page.attribute_value(frame, "srcdoc").is_none()
        && vm.page.attribute_value(frame, "src").is_some_and(|src| {
            let src = src.trim();
            !src.is_empty() && !src.eq_ignore_ascii_case("about:blank")
        })
}

/** The frame's window: its document, the frame element, and the page as parent and top. It shares
the page's realm, so its interface constructors and DOMException come from its prototype, and its
own members shadow the page's. Created once per frame and kept, so identity holds. */
pub(super) fn content_window(vm: &mut Vm, frame: usize, document: usize) -> Res<JsValue> {
    if let Some(id) = vm.page.state.frame_windows.get(&frame)
        && vm.heap.get(*id).is_some()
    {
        return Ok(JsValue::Object(*id));
    }
    let interfaces = vm.intrinsic_id(realm().ids.window_interfaces);
    vm.materialize(interfaces)?;
    let id = vm.alloc(Object::new(Class::Plain, Some(interfaces)))?;
    let document = wrap_document(vm, document)?;
    let element = wrap(vm, frame)?;
    vm.put(id, "document", document)?;
    vm.put(id, "frameElement", element)?;
    for name in ["parent", "top"] {
        vm.put(id, name, JsValue::Object(GLOBAL))?;
    }
    vm.put(id, "window", JsValue::Object(id))?;
    vm.put(id, "self", JsValue::Object(id))?;
    vm.put(id, "globalThis", JsValue::Object(id))?;
    vm.page.state.frame_windows.insert(frame, id);
    Ok(JsValue::Object(id))
}

/** The media type of an HTML document, which the page and every HTML document report. */
pub(super) const HTML_TYPE: &str = "text/html";
/** The media type of an XHTML document, which createDocument gives the HTML namespace. */
pub(super) const XHTML_TYPE: &str = "application/xhtml+xml";
/** The media type of an SVG document, which createDocument gives the SVG namespace. */
pub(super) const SVG_TYPE: &str = "image/svg+xml";
/** The media type of every other XML document: a new Document and the rest of createDocument. */
pub(super) const XML_TYPE: &str = "application/xml";
/** The URL of a document with no address of its own: script-made and frame documents. */
pub(super) const ABOUT_BLANK: &str = "about:blank";

/** Give a clone of a document the media type and URL its source had, as cloneNode keeps both. */
pub(super) fn copy_document_kind(vm: &mut Vm, from: usize, to: usize) {
    if let Some(media) = vm.page.state.content_types.get(&from).cloned() {
        vm.page.state.content_types.insert(to, media);
    }
    if vm.page.state.parsed_documents.contains(&from) {
        vm.page.state.parsed_documents.insert(to);
    }
}

/** The URL a document reports: a DOMParser document has the page's address, and every other
script-made or frame document is about:blank. */
pub(super) fn document_url(vm: &Vm, document: usize) -> String {
    if vm.page.state.parsed_documents.contains(&document) {
        vm.page.state.href.clone()
    } else {
        ABOUT_BLANK.to_string()
    }
}

/** The document's element child and the html child named `tag` (head or body). */
pub(super) fn document_part(vm: &mut Vm, document: usize, tag: &str) -> Option<usize> {
    let root = vm
        .page
        .children(document)
        .into_iter()
        .find(|node| vm.page.is_element(*node))?;
    if tag == "html" {
        return Some(root);
    }
    vm.page
        .children(root)
        .into_iter()
        .find(|node| vm.page.tag(*node) == tag)
}

/** Document members of a script-made or frame document that differ from the page's own. */
pub(super) fn document_get(vm: &mut Vm, document: usize, call: Call) -> Res<JsValue> {
    match native_name(&call) {
        "documentElement" => {
            let root = document_part(vm, document, "html");
            wrap_option(vm, root)
        }
        name @ ("head" | "body") => {
            let part = document_part(vm, document, name);
            wrap_option(vm, part)
        }
        "childNodes" | "firstChild" | "lastChild" => dom::node_get(vm, call),
        "children" | "firstElementChild" | "lastElementChild" => dom::element_get(vm, call),
        "readyState" => JsValue::string("complete").map_err(Into::into),
        "contentType" => JsValue::string(&content_type_of(vm, document)).map_err(Into::into),
        "URL" | "documentURI" | "baseURI" => {
            JsValue::string(&document_url(vm, document)).map_err(Into::into)
        }
        "title" => {
            let title = dom::search(vm, document, |page, id| page.tag(id) == "title");
            let text = title
                .first()
                .map(|node| vm.page.text_content(*node))
                .unwrap_or_default();
            JsValue::string(&text.split_ascii_whitespace().collect::<Vec<_>>().join(" "))
                .map_err(Into::into)
        }
        _ => Ok(JsValue::Undefined),
    }
}

/** `document.doctype` of any document. */
pub(super) fn doctype_of(vm: &mut Vm, document: usize) -> Res<JsValue> {
    let found = vm
        .page
        .children(document)
        .into_iter()
        .find(|node| vm.page.tag(*node) == DOCTYPE);
    wrap_option(vm, found)
}

/** Document methods of a script-made or frame document; None leaves the call to the page
document's own steps (node creation, events and the like). */
pub(super) fn document_method(vm: &mut Vm, document: usize, call: &Call) -> Res<Option<JsValue>> {
    let name = native_name(call);
    let value = match name {
        "open" => {
            open(vm, document)?;
            call.this.clone()
        }
        "write" | "writeln" => {
            let mut text = String::new();
            for arg in call.args {
                text.push_str(&vm.to_text(arg)?);
            }
            if name == "writeln" {
                text.push('\n');
            }
            write(vm, document, &text)?;
            JsValue::Undefined
        }
        "close" => {
            close(vm, document)?;
            JsValue::Undefined
        }
        "getElementById" => {
            let wanted = vm.to_text(&call.arg(0))?;
            let found = dom::search(vm, document, |page, id| {
                !wanted.is_empty()
                    && page
                        .view(id)
                        .is_some_and(|view| view.attributes.get("id") == Some(&wanted))
            });
            wrap_option(vm, found.first().copied())?
        }
        "querySelector" | "querySelectorAll" => dom::query(vm, document, call)?,
        "getElementsByTagName" | "getElementsByTagNameNS" | "getElementsByClassName" => {
            let Some(root) = document_part(vm, document, "html") else {
                return dom::node_list(vm, Vec::new()).map(Some);
            };
            let root = wrap(vm, root)?;
            let method = vm.get(&root, &crate::v9::vm::heap::Key::from(name))?;
            vm.call(method, root, call.args.to_vec())?
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

/** Document::open: drop the document's children and start a fresh write buffer. */
fn open(vm: &mut Vm, document: usize) -> Res<()> {
    for child in vm.page.children(document) {
        vm.page
            .remove(child)
            .map_err(|error| dom_error(vm, error))?;
    }
    vm.page.state.writes.insert(document, String::new());
    Ok(())
}

/** Document::write: a write to a document with no open parser opens it first. */
fn write(vm: &mut Vm, document: usize, text: &str) -> Res<()> {
    if !vm.page.state.writes.contains_key(&document) {
        open(vm, document)?;
    }
    let limit = crate::constants::FRAGMENT_BYTES * 4;
    let buffer = vm.page.state.writes.entry(document).or_default();
    if buffer.len() + text.len() > limit {
        return Err(dom_error(
            vm,
            crate::protocol::EngineError::limit("document.write bytes", limit),
        ));
    }
    buffer.push_str(text);
    Ok(())
}

/** Document::close: parse what was written, then fire the frame's load event. */
fn close(vm: &mut Vm, document: usize) -> Res<()> {
    let Some(markup) = vm.page.state.writes.remove(&document) else {
        return Ok(());
    };
    build(vm, document, &markup)?;
    let frame = vm.page.state.document_frames.get(&document).copied();
    if let Some(frame) = frame {
        super::fire(vm, super::Target::Node(frame), "load", false)?;
    }
    Ok(())
}

/** Where the tree builder is: the insertion modes that decide a node's parent. */
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Mode {
    BeforeHtml,
    BeforeHead,
    InHead,
    AfterHead,
    InBody,
}

/** The document being built and where its next node goes. */
struct DocumentBuilder {
    document: usize,
    html: usize,
    head: usize,
    body: usize,
    mode: Mode,
    parsed: Vec<Parsed>,
    children: Vec<Vec<usize>>,
}

/** Parse `markup` as a whole document into the (empty) document node. */
pub(super) fn build(vm: &mut Vm, document: usize, markup: &str) -> Res<()> {
    let rest = prologue(vm, document, markup)?;
    let parsed = parse_tree(rest).map_err(|error| dom_error(vm, error))?;
    let mut children = vec![Vec::new(); parsed.len()];
    let mut roots = Vec::new();
    for (index, node) in parsed.iter().enumerate() {
        match node.parent {
            Some(parent) => children[parent].push(index),
            None => roots.push(index),
        }
    }
    let html = create(vm, "html", None)?;
    let head = create(vm, "head", None)?;
    let body = create(vm, "body", None)?;
    let mut tree = DocumentBuilder {
        document,
        html,
        head,
        body,
        mode: Mode::BeforeHtml,
        parsed,
        children,
    };
    append(vm, document, html)?;
    append(vm, html, head)?;
    for index in roots {
        tree.place(vm, index)?;
    }
    if vm.page.parent(body).is_none() {
        append(vm, html, body)?;
    }
    Ok(())
}

/** The initial insertion mode: leading whitespace is dropped, comments and the doctype go to
the document. Returns the markup after them. */
fn prologue<'m>(vm: &mut Vm, document: usize, markup: &'m str) -> Res<&'m str> {
    let mut rest = markup;
    loop {
        rest = rest.trim_start_matches(['\t', '\n', '\x0C', '\r', ' ']);
        if let Some(after) = rest.strip_prefix("<!--") {
            let end = after.find("-->").unwrap_or(after.len());
            let comment = create(vm, "#comment", Some(after[..end].to_string()))?;
            append(vm, document, comment)?;
            rest = after.get(end + 3..).unwrap_or("");
            continue;
        }
        if rest.len() >= 9 && rest[..9].eq_ignore_ascii_case("<!doctype") {
            let end = rest.find('>').unwrap_or(rest.len());
            let (name, public, system) = doctype_parts(&rest[9..end]);
            let node = doctype(vm, &name, &public, &system)?;
            append(vm, document, node)?;
            rest = rest.get(end + 1..).unwrap_or("");
            continue;
        }
        return Ok(rest);
    }
}

/** A doctype's name (lower-cased) and its public and system identifiers. */
fn doctype_parts(body: &str) -> (String, String, String) {
    let body = body.trim_start();
    let end = body
        .find(|c: char| c.is_ascii_whitespace())
        .unwrap_or(body.len());
    let name = body[..end].to_ascii_lowercase();
    let rest = body[end..].trim_start();
    let keyword = |word: &str| rest.len() >= 6 && rest[..6].eq_ignore_ascii_case(word);
    let mut quoted = quoted_strings(&rest[rest.len().min(6)..]).into_iter();
    if keyword("public") {
        let public = quoted.next().unwrap_or_default();
        return (name, public, quoted.next().unwrap_or_default());
    }
    if keyword("system") {
        return (name, String::new(), quoted.next().unwrap_or_default());
    }
    (name, String::new(), String::new())
}

/** The single- or double-quoted strings in a doctype's identifier part, in order. */
fn quoted_strings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(['"', '\'']) {
        let quote = rest[start..].chars().next().unwrap_or('"');
        let after = &rest[start + 1..];
        let end = after.find(quote).unwrap_or(after.len());
        out.push(after[..end].to_string());
        rest = after.get(end + 1..).unwrap_or("");
    }
    out
}

impl DocumentBuilder {
    /** Place one parsed top-level node by the current insertion mode. */
    fn place(&mut self, vm: &mut Vm, index: usize) -> Res<()> {
        let tag = self.parsed[index].tag.clone();
        match tag.as_str() {
            "html" => {
                self.mode = self.mode.max(Mode::BeforeHead);
                self.merge(vm, self.html, index);
                self.flatten(vm, index)
            }
            "head" if self.mode < Mode::InBody => {
                self.merge(vm, self.head, index);
                self.mode = Mode::InHead;
                self.flatten(vm, index)?;
                self.mode = self.mode.max(Mode::AfterHead);
                Ok(())
            }
            "head" => self.flatten(vm, index),
            "body" => {
                self.enter_body(vm)?;
                self.merge(vm, self.body, index);
                self.flatten(vm, index)
            }
            "#comment" => {
                let parent = match self.mode {
                    Mode::BeforeHtml => self.document,
                    Mode::BeforeHead => self.html,
                    Mode::InHead => self.head,
                    Mode::AfterHead => self.html,
                    Mode::InBody => self.body,
                };
                self.copy(vm, index, parent)
            }
            "#text" => self.text(vm, index),
            _ if self.mode < Mode::InBody && HEAD_CONTENT.contains(&tag.as_str()) => {
                self.mode = self.mode.max(Mode::InHead);
                self.copy(vm, index, self.head)
            }
            _ => {
                self.enter_body(vm)?;
                self.copy(vm, index, self.body)
            }
        }
    }

    /** Place an implied container's children as if they were top-level. */
    fn flatten(&mut self, vm: &mut Vm, index: usize) -> Res<()> {
        for child in self.children[index].clone() {
            self.place(vm, child)?;
        }
        Ok(())
    }

    /** Attributes of an explicit html, head or body tag the implied element lacks. */
    fn merge(&self, vm: &mut Vm, element: usize, index: usize) {
        if let Some(record) = vm.page.state.detached.get_mut(&element) {
            for (name, value) in &self.parsed[index].attributes {
                record
                    .attributes
                    .entry(name.clone())
                    .or_insert_with(|| value.clone());
            }
        }
    }

    /** Insert the body once, when the first body content arrives. */
    fn enter_body(&mut self, vm: &mut Vm) -> Res<()> {
        if self.mode < Mode::InBody {
            self.mode = Mode::InBody;
            append(vm, self.html, self.body)?;
        }
        Ok(())
    }

    /** Text: whitespace before the body stays in head or html (or is dropped before them);
    the rest starts the body. */
    fn text(&mut self, vm: &mut Vm, index: usize) -> Res<()> {
        let text = self.parsed[index].text.clone().unwrap_or_default();
        if self.mode == Mode::InBody {
            return self.copy(vm, index, self.body);
        }
        let blank = text
            .find(|c: char| !matches!(c, '\t' | '\n' | '\x0C' | '\r' | ' '))
            .unwrap_or(text.len());
        let parent = match self.mode {
            Mode::InHead => Some(self.head),
            Mode::AfterHead => Some(self.html),
            _ => None,
        };
        if let Some(parent) = parent.filter(|_| blank > 0) {
            let node = create(vm, "#text", Some(text[..blank].to_string()))?;
            append(vm, parent, node)?;
        }
        if blank < text.len() {
            self.enter_body(vm)?;
            let node = create(vm, "#text", Some(text[blank..].to_string()))?;
            append(vm, self.body, node)?;
        }
        Ok(())
    }

    /** Copy a parsed node and its subtree under `parent`. */
    fn copy(&self, vm: &mut Vm, index: usize, parent: usize) -> Res<()> {
        let parsed = &self.parsed[index];
        let node = create(vm, &parsed.tag, parsed.text.clone())?;
        if let Some(record) = vm.page.state.detached.get_mut(&node) {
            record.attributes.clone_from(&parsed.attributes);
        }
        append(vm, parent, node)?;
        for child in &self.children[index] {
            self.copy(vm, *child, node)?;
        }
        Ok(())
    }
}
