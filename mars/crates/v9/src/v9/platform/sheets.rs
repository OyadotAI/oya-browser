/**
 * `document.styleSheets`, a style or link element's `sheet`, and form validity: the objects a
 * page reads to find the style sheet it just inserted or to check a field before submitting.
 * V9 has no cascade, so a sheet keeps its rules as the element's text. `insertRule` and
 * `deleteRule` edit the sheet's list of top-level rules (see `sheet_list`), which is written back
 * to the text before Eye reads it (Eye picks the change up as a sheet change), so a nested
 * `@media` block survives an edit around it. Every field is valid, since V9 runs no constraint
 * validation.
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
    vm.page.flush_sheet_edits();
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

/** The sheet's rules as `{cssText, selectorText, style, type}` records, one per top-level rule. */
fn rules(vm: &mut Vm, node: usize) -> Res<JsValue> {
    let blocks = vm.page.sheet_list(node).rule_texts();
    let mut out = Vec::new();
    for block in blocks {
        let block = block.as_str();
        let rule = vm.new_object()?;
        vm.put(rule, "cssText", JsValue::string(block)?)?;
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
        "insertRule" => {
            let rule = vm.to_text(&call.arg(0))?;
            let index = vm.to_number(&call.arg(1))?;
            insert_rule(vm, node, &rule, Some(index))
        }
        "addRule" => {
            let selector = vm.to_text(&call.arg(0))?;
            let body = vm.to_text(&call.arg(1))?;
            let rule = format!("{selector} {{ {body} }}");
            let index = match call.arg(2) {
                JsValue::Undefined => None,
                value => Some(vm.to_number(&value)?),
            };
            insert_rule(vm, node, &rule, index)
        }
        "deleteRule" | "removeRule" => {
            let index =
                if dom::native_name(&call) == "removeRule" && call.arg(0) == JsValue::Undefined {
                    0.0
                } else {
                    vm.to_number(&call.arg(0))?
                };
            delete_rule(vm, node, index)?;
            Ok(JsValue::Undefined)
        }
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

/** The rule index a sheet method's argument names, WebIDL style: NaN counts as 0 and fractions
truncate. Returns None past `len`, which callers turn into an IndexSizeError. */
fn sheet_index(index: f64, len: usize) -> Option<usize> {
    let whole = if index.is_nan() { 0.0 } else { index.trunc() };
    (whole >= 0.0 && whole <= len as f64).then_some(whole as usize)
}

/** Insert `rule` before the top-level rule at `index`, or append it when `index` is None or
equals the rule count, and return the index it landed at. The text around the insertion point is
kept as it was, so nested blocks and comments stay whole. An index past the rule count is an
IndexSizeError. */
fn insert_rule(vm: &mut Vm, node: usize, rule: &str, index: Option<f64>) -> Res<JsValue> {
    let len = vm.page.sheet_list(node).len();
    let position = match index {
        None => len,
        Some(index) => sheet_index(index, len).ok_or_else(|| index_error(vm))?,
    };
    vm.page.edit_sheet(node, |list| list.insert(position, rule));
    Ok(JsValue::Number(position as f64))
}

/** Remove the top-level rule at `index`, keeping the text around it as it was. An index outside
the rules is an IndexSizeError, which a page's own cleanup code relies on to fail loudly. */
fn delete_rule(vm: &mut Vm, node: usize, index: f64) -> Res<()> {
    let len = vm.page.sheet_list(node).len();
    let position = sheet_index(index, len)
        .filter(|position| *position < len)
        .ok_or_else(|| index_error(vm))?;
    vm.page.edit_sheet(node, |list| list.remove(position));
    Ok(())
}

/** The IndexSizeError a sheet method throws for an index outside its rules. */
fn index_error(vm: &mut Vm) -> crate::v9::vm::Thrown {
    vm.dom_exception("IndexSizeError", "The index is outside the sheet's rules.")
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
