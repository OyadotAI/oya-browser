use super::Page;
use super::components::FRAGMENT;
/**
 * XMLSerializer: `serializeToString` writes a node as XML, as the DOM Parsing standard describes.
 * Elements carry their namespace: a namespace that differs from the one the enclosing element set
 * is declared with xmlns, and an empty element is written self-closing. Text, comments,
 * processing instructions and doctypes are escaped or written as XML requires. The output is
 * what the XML reader parses back, so a round trip keeps the tree.
 */
use super::dom;
use super::frames::{DOCTYPE, DOCUMENT_NODE};
use super::implementation::PROCESSING_INSTRUCTION;
use super::namespaces::implied_namespace;
use super::nodes::NodeView;
use crate::v9::value::{JsString, JsValue};
use crate::v9::vm::realm::{Builder, realm};
use crate::v9::vm::{Call, Res, Vm};

/** The namespace an element is in, or None for the null namespace. */
type Namespace = Option<String>;

/** Install the XMLSerializer constructor and its prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.host_method(proto, "serializeToString", 1, serialize_to_string);
    let ctor = b.constructor("XMLSerializer", 0, construct, proto, None);
    b.global("XMLSerializer", ctor);
}

/** `new XMLSerializer()`: a plain object over the prototype; it holds no state. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let fallback = realm().ids.object_prototype;
    let proto = match &call.new_target {
        Some(target) => vm.prototype_for(target, fallback)?,
        None => return Err(vm.type_error("Constructor XMLSerializer requires 'new'")),
    };
    let id = vm.alloc(crate::v9::vm::heap::Object::new(
        crate::v9::vm::data::Class::Plain,
        Some(proto),
    ))?;
    Ok(JsValue::Object(id))
}

/** `serializeToString(node)`: the node's XML. The argument must be a node, as WebIDL requires. */
fn serialize_to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(node) = dom::node_of(vm, &call.arg(0)) else {
        return Err(vm.type_error(
            "Failed to execute 'serializeToString' on 'XMLSerializer': parameter 1 is not of type 'Node'.",
        ));
    };
    let mut out = String::new();
    write_node(&mut vm.page, node, &None, 0, &mut out);
    Ok(JsValue::String(JsString::from_utf8(&out)?))
}

/** How deep the writer descends: the bound the HTML serializer uses, so a deep tree cannot exhaust the stack. */
const MAX_DEPTH: usize = crate::constants::DOCUMENT_DEPTH * 4;

/** Write one node and its subtree. `inherited` is the namespace the enclosing element set. */
fn write_node(page: &mut Page, id: usize, inherited: &Namespace, depth: usize, out: &mut String) {
    let Some(view) = page.view(id) else {
        return;
    };
    if depth > MAX_DEPTH {
        return;
    }
    match view.tag.as_str() {
        "#text" => escape_text(view.text.as_deref().unwrap_or(""), out),
        "#comment" => push_wrapped(out, "<!--", view.text.as_deref().unwrap_or(""), "-->"),
        PROCESSING_INSTRUCTION => write_processing_instruction(&view, out),
        DOCTYPE => write_doctype(&view, out),
        DOCUMENT_NODE | FRAGMENT => write_children(page, id, &None, depth, out),
        _ => write_element(page, id, &view.tag, &view.attributes, inherited, depth, out),
    }
}

/** Write `prefix` + `body` + `suffix`; the helper keeps the markup-wrapping cases to one line. */
fn push_wrapped(out: &mut String, prefix: &str, body: &str, suffix: &str) {
    out.push_str(prefix);
    out.push_str(body);
    out.push_str(suffix);
}

/** Write a processing instruction as `<?target data?>`, leaving out the space when there is no data. */
fn write_processing_instruction(view: &NodeView, out: &mut String) {
    let target = view.attributes.get("target").map_or("", String::as_str);
    let data = view.text.as_deref().unwrap_or("");
    out.push_str("<?");
    out.push_str(target);
    if !data.is_empty() {
        out.push(' ');
        out.push_str(data);
    }
    out.push_str("?>");
}

/** Write a doctype as `<!DOCTYPE name>`. */
fn write_doctype(view: &NodeView, out: &mut String) {
    let name = view.attributes.get("name").map_or("", String::as_str);
    push_wrapped(out, "<!DOCTYPE ", name, ">");
}

/** Write an element: its start tag, its attributes, its children, and its end tag. */
fn write_element(
    page: &mut Page,
    id: usize,
    tag: &str,
    attributes: &std::collections::BTreeMap<String, String>,
    inherited: &Namespace,
    depth: usize,
    out: &mut String,
) {
    let (namespace, name) = element_name(page, id, tag);
    out.push('<');
    out.push_str(&name);
    write_namespace_declaration(&namespace, inherited, out);
    for (attribute, value) in attributes
        .iter()
        .filter(|(name, _)| name.as_str() != "xmlns")
    {
        out.push(' ');
        out.push_str(attribute);
        out.push_str("=\"");
        escape_attribute(value, out);
        out.push('"');
    }
    if page.children(id).is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    write_children(page, id, &namespace, depth, out);
    push_wrapped(out, "</", &name, ">");
}

/** The namespace and qualified name an element is written under. Script-made elements carry their
record; parsed elements take the namespace their tag implies. */
fn element_name(page: &mut Page, id: usize, tag: &str) -> (Namespace, String) {
    match page.state.element_ns.get(&id).cloned() {
        Some(record) => {
            let name = match &record.prefix {
                Some(prefix) => format!("{prefix}:{}", record.local),
                None => record.local.clone(),
            };
            (record.namespace, name)
        }
        None => (Some(implied_namespace(tag).to_string()), tag.to_string()),
    }
}

/** Declare a namespace when the element's differs from the one its parent set: `xmlns` names it,
and an element in the null namespace inside a namespaced parent declares `xmlns=""`. */
fn write_namespace_declaration(namespace: &Namespace, inherited: &Namespace, out: &mut String) {
    if namespace == inherited {
        return;
    }
    out.push_str(" xmlns=\"");
    out.push_str(namespace.as_deref().unwrap_or(""));
    out.push('"');
}

/** Write every child of a node in order, each in the namespace its parent element set. */
fn write_children(
    page: &mut Page,
    id: usize,
    namespace: &Namespace,
    depth: usize,
    out: &mut String,
) {
    for child in page.children(id) {
        write_node(page, child, namespace, depth + 1, out);
    }
}

/** Escape character data: the ampersand, the angle brackets, and a carriage return. */
fn escape_text(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#xD;"),
            c => out.push(c),
        }
    }
}

/** Escape an attribute value, which is quoted with double quotes: the value's markup characters and
its whitespace controls are written as references, so the value reads back unchanged. */
fn escape_attribute(text: &str, out: &mut String) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#x9;"),
            '\n' => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            c => out.push(c),
        }
    }
}
