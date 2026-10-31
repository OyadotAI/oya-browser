/**
 * `document.styleSheets`, a style or link element's `sheet`, and form validity: the objects a
 * page reads to find the style sheet it just inserted or to check a field before submitting.
 * V9 has no cascade, so a sheet lists its rules by splitting the element's text, and
 * `insertRule` appends to that text (Eye picks the change up as a sheet change); every field
 * is valid, since V9 runs no constraint validation.
 */
use super::dom::{self, DOCUMENT, wrap};
use super::host_data;
use crate::v9::value::JsValue;
use crate::v9::vm::data::{Class, HostData};
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::realm;
use crate::v9::vm::{Call, Res, Vm};

/** The style-sheet-bearing elements of the document, in document order. */
fn sheet_nodes(vm: &mut Vm) -> Vec<usize> {
    dom::search(vm, DOCUMENT, |page, id| {
        page.view(id).is_some_and(|view| {
            view.tag == "style"
                || view.tag == "link"
                    && view.attributes.get("rel").is_some_and(|rel| {
                        rel.split_ascii_whitespace()
                            .any(|word| word.eq_ignore_ascii_case("stylesheet"))
                    })
        })
    })
}

/** A stable hash of a style text; the same text gives the same value in every process. */
pub(in crate::v9) fn text_hash(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/** Report the texts of the V9-only `<style>` elements the app does not already apply, when that
list changed since the last report. The document's own parsed head styles are skipped until a
script changes their text. Runs at the end of every step. */
pub(in crate::v9) fn publish_local(vm: &mut Vm) {
    if let Some((hash, texts)) = local_texts(vm) {
        vm.page.state.local_sheets_hash = hash;
        vm.page.effects.local_sheets = Some(texts);
    }
}

/** The V9-only style texts when they changed since the last report, without reporting them:
what a synchronous layout mid-step must style with. */
pub(in crate::v9) fn changed_local(vm: &mut Vm) -> Option<Vec<String>> {
    local_texts(vm).map(|(_, texts)| texts)
}

/** The V9-only `<style>` texts the app does not already apply, with their combined hash, when
that hash differs from the last reported one. */
fn local_texts(vm: &mut Vm) -> Option<(u64, Vec<String>)> {
    let mut texts = Vec::new();
    for node in sheet_nodes(vm) {
        if vm.page.tag(node) != "style" || vm.page.is_projected(node) {
            continue;
        }
        let text = vm.page.text_content(node);
        let parsed = vm.page.state.parsed_style_hashes.get(&node).copied();
        if parsed != Some(text_hash(&text)) {
            texts.push(text);
        }
    }
    let hash = text_hash(&texts.join("\u{0}"));
    (hash != vm.page.state.local_sheets_hash).then_some((hash, texts))
}

/** The CSSStyleSheet object of a node. */
fn sheet_object(vm: &mut Vm, node: usize) -> Res<JsValue> {
    let id = vm.alloc(Object::new(
        Class::Host(Box::new(HostData::StyleSheet(node))),
        Some(realm().ids.style_sheet_prototype),
    ))?;
    Ok(JsValue::Object(id))
}

/** `document.styleSheets`: a fresh list of the document's sheets. */
pub(in crate::v9) fn style_sheets(vm: &mut Vm) -> Res<JsValue> {
    let mut sheets = Vec::new();
    for node in sheet_nodes(vm) {
        sheets.push(sheet_object(vm, node)?);
    }
    let list = vm.new_array(sheets)?;
    vm.set_proto(list, Some(realm().ids.named_node_map_prototype))?;
    Ok(JsValue::Object(list))
}

/** `element.sheet`: the sheet of a style or stylesheet link element, else null. */
pub(in crate::v9) fn element_sheet(vm: &mut Vm, node: usize) -> Res<JsValue> {
    if sheet_nodes(vm).contains(&node) {
        sheet_object(vm, node)
    } else {
        Ok(JsValue::Null)
    }
}

/** The node behind a sheet receiver. */
fn sheet_node(vm: &mut Vm, call: &Call) -> Res<usize> {
    match host_data(vm, &call.this) {
        Some(HostData::StyleSheet(node)) => Ok(node),
        _ => Err(vm.type_error("Illegal invocation")),
    }
}

/** The sheet's rules as `{cssText, selectorText, style, type}` records, one per `}`-ended block. */
fn rules(vm: &mut Vm, node: usize) -> Res<JsValue> {
    let text = vm.page.text_content(node);
    let mut out = Vec::new();
    for block in text.split('}') {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        let rule = vm.new_object()?;
        vm.put(rule, "cssText", JsValue::string(&format!("{block} }}"))?)?;
        let selector = block.split('{').next().unwrap_or("").trim();
        vm.put(rule, "selectorText", JsValue::string(selector)?)?;
        vm.put(
            rule,
            "type",
            JsValue::Number(if selector.starts_with('@') { 0.0 } else { 1.0 }),
        )?;
        let style = vm.new_object()?;
        vm.put(rule, "style", JsValue::Object(style))?;
        out.push(JsValue::Object(rule));
    }
    let list = vm.new_array(out)?;
    vm.set_proto(list, Some(realm().ids.named_node_map_prototype))?;
    Ok(JsValue::Object(list))
}

/** CSSStyleSheet accessors. */
pub(in crate::v9) fn sheet_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = sheet_node(vm, &call)?;
    let view = vm.page.view(node);
    Ok(match dom::native_name(&call) {
        "ownerNode" => wrap(vm, node)?,
        "href" => match view.and_then(|view| view.attributes.get("href").cloned()) {
            Some(href) => JsValue::string(&super::net::resolve(vm, &href).unwrap_or(href))?,
            None => JsValue::Null,
        },
        "disabled" => JsValue::Boolean(false),
        "type" => JsValue::string("text/css")?,
        "title" => match view.and_then(|view| view.attributes.get("title").cloned()) {
            Some(title) => JsValue::string(&title)?,
            None => JsValue::Null,
        },
        "media" => {
            let media = vm.new_object()?;
            let text = view
                .and_then(|view| view.attributes.get("media").cloned())
                .unwrap_or_default();
            vm.put(media, "mediaText", JsValue::string(&text)?)?;
            vm.put(media, "length", JsValue::Number(0.0))?;
            JsValue::Object(media)
        }
        "cssRules" | "rules" => rules(vm, node)?,
        "parentStyleSheet" | "ownerRule" => JsValue::Null,
        _ => JsValue::Undefined,
    })
}

/** CSSStyleSheet methods: `insertRule` appends to the element's text; deletion is unsupported. */
pub(in crate::v9) fn sheet_method(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = sheet_node(vm, &call)?;
    match dom::native_name(&call) {
        "insertRule" | "addRule" => {
            let rule = if dom::native_name(&call) == "addRule" {
                let selector = vm.to_text(&call.arg(0))?;
                let body = vm.to_text(&call.arg(1))?;
                format!("{selector} {{ {body} }}")
            } else {
                vm.to_text(&call.arg(0))?
            };
            let mut text = vm.page.text_content(node);
            text.push('\n');
            text.push_str(&rule);
            vm.page
                .set_text(node, &text)
                .map_err(|error| dom::dom_error(vm, error))?;
            let count = text
                .split('}')
                .filter(|block| !block.trim().is_empty())
                .count();
            Ok(JsValue::Number(count.saturating_sub(1) as f64))
        }
        "deleteRule" | "removeRule" => Err(vm.unsupported_error("CSSStyleSheet rule deletion")),
        "replaceSync" | "replace" => {
            let text = vm.to_text(&call.arg(0))?;
            vm.page
                .set_text(node, &text)
                .map_err(|error| dom::dom_error(vm, error))?;
            Ok(JsValue::Undefined)
        }
        _ => Ok(JsValue::Undefined),
    }
}

/** `element.validity`: a ValidityState in which every field is valid. */
pub(in crate::v9) fn validity(vm: &mut Vm) -> Res<JsValue> {
    let id = vm.new_object()?;
    for name in [
        "badInput",
        "customError",
        "patternMismatch",
        "rangeOverflow",
        "rangeUnderflow",
        "stepMismatch",
        "tooLong",
        "tooShort",
        "typeMismatch",
        "valueMissing",
    ] {
        vm.put(id, name, JsValue::Boolean(false))?;
    }
    vm.put(id, "valid", JsValue::Boolean(true))?;
    Ok(JsValue::Object(id))
}
