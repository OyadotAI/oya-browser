/**
 * DOMParser: `parseFromString` builds an inert tree from markup, as a parsed document does, and
 * returns it with the document members pages read (`documentElement`, `firstChild`, and for
 * HTML `head` and `body`). LinkedIn turns each fetched SVG icon into an element this way and
 * appends it; without DOMParser every icon placeholder stayed empty.
 *
 * The tree is the content of an inert template, so nothing in it runs or upgrades, and its
 * nodes adopt into the page when appended. Markup goes through the HTML parser for every type;
 * an XML declaration is dropped first, since it is not a node.
 */
use super::dom::{self, set_inner_html, wrap};
use crate::v9::value::JsValue;
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};

/** Install the DOMParser constructor and its prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.method(proto, "parseFromString", 2, parse_from_string);
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

/** `parseFromString(markup, type)`: the parsed tree as a document-like fragment. */
fn parse_from_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let markup = vm.to_text(&call.arg(0))?;
    let html = vm.to_text(&call.arg(1))?.eq_ignore_ascii_case("text/html");
    let template = create(vm, "template")?;
    let content = vm
        .page
        .content_or_self(template)
        .map_err(|error| dom::dom_error(vm, error))?;
    let parts = if html {
        Some(html_skeleton(vm, content, &markup)?)
    } else {
        set_inner_html(vm, template, without_declaration(&markup))?;
        None
    };
    let document = wrap(vm, content)?;
    let JsValue::Object(id) = document else {
        return Err(vm.type_error("DOMParser could not build the document"));
    };
    let root = match node_of_root(vm, content) {
        Some(node) => wrap(vm, node)?,
        None => JsValue::Null,
    };
    vm.put(id, "documentElement", root)?;
    if let Some((head, body)) = parts {
        let head = wrap(vm, head)?;
        vm.put(id, "head", head)?;
        let body = wrap(vm, body)?;
        vm.put(id, "body", body)?;
    }
    Ok(document)
}

/** A detached element of `tag`. */
fn create(vm: &mut Vm, tag: &str) -> Res<usize> {
    vm.page
        .create(tag, None)
        .map_err(|error| dom::dom_error(vm, error))
}

/** An html element with an empty head and a body holding the parsed markup, under `content`. */
fn html_skeleton(vm: &mut Vm, content: usize, markup: &str) -> Res<(usize, usize)> {
    let root = create(vm, "html")?;
    let head = create(vm, "head")?;
    let body = create(vm, "body")?;
    for (parent, child) in [(content, root), (root, head), (root, body)] {
        vm.page
            .insert(parent, child, None)
            .map_err(|error| dom::dom_error(vm, error))?;
    }
    set_inner_html(vm, body, markup)?;
    Ok((head, body))
}

/** The fragment's first element. */
fn node_of_root(vm: &mut Vm, content: usize) -> Option<usize> {
    vm.page
        .children(content)
        .into_iter()
        .find(|node| !vm.page.tag(*node).starts_with('#'))
}

/** Markup without a leading XML declaration (`<?xml ...?>`), which is not a node. */
fn without_declaration(markup: &str) -> &str {
    let trimmed = markup.trim_start();
    if trimmed.starts_with("<?xml")
        && let Some(end) = trimmed.find("?>")
    {
        return trimmed[end + 2..].trim_start();
    }
    markup
}
