/**
 * ProcessingInstruction's attribute methods. A processing instruction keeps no attribute table:
 * its data is the source of truth, read as `name="value"` pairs (the xml-stylesheet grammar) in
 * the order they were written, and each change rewrites the data as one characterData record.
 * Keeping the text canonical means `data` and the attribute methods can never disagree.
 */
use super::components;
use super::dom::{self, set_data};
use super::html::escape_attribute;
use crate::v9::value::JsValue;
use crate::v9::vm::{Call, Res, Vm};

/** The attribute list a processing instruction's data reads as, in the order written. */
type Pairs = Vec<(String, String)>;
/** The DOM error name for an attribute name the pseudo-attribute grammar rejects. */
const INVALID_CHARACTER: &str = "InvalidCharacterError";
/** Characters a pseudo-attribute name may not contain, besides whitespace. */
const NAME_STOPS: [char; 6] = ['=', '/', '>', '?', '"', '\''];
/** The quotes a pseudo-attribute value may be wrapped in. */
const QUOTES: [char; 2] = ['"', '\''];

/** Whether a character may appear in a pseudo-attribute name. */
fn is_name_char(c: char) -> bool {
    !c.is_whitespace() && !NAME_STOPS.contains(&c)
}

/** Whether a name is usable as a pseudo-attribute: non-empty and free of whitespace and the
stop characters, so `$`, `_` and `a$b` are names while `a=`, `/x` and `x\t` are not. */
pub(super) fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(is_name_char)
}

/** The pairs in some data, in order. Parsing stops at the first thing that is not `name="value"`,
so an unquoted value drops it and everything after it, as the grammar requires. */
pub(super) fn parse(data: &str) -> Pairs {
    let mut rest = data.trim_start();
    let mut pairs = Vec::new();
    while let Some((pair, after)) = parse_pair(rest) {
        pairs.push(pair);
        rest = after.trim_start();
    }
    pairs
}

/** One `name="value"` at the start of `text`, and the text that follows it. */
fn parse_pair(text: &str) -> Option<((String, String), &str)> {
    let name_end = text.find(|c: char| !is_name_char(c)).unwrap_or(text.len());
    let (name, after) = text.split_at(name_end);
    if name.is_empty() {
        return None;
    }
    let value_text = after.strip_prefix('=')?;
    let quote = value_text.chars().next().filter(|c| QUOTES.contains(c))?;
    let body = &value_text[quote.len_utf8()..];
    let close = body.find(quote)?;
    let pair = (name.to_string(), body[..close].to_string());
    Some((pair, &body[close + quote.len_utf8()..]))
}

/** The data for a list of pairs: `name="value"` joined by spaces, values escaped as attributes
are, so reading the data back yields the same values. */
pub(super) fn serialize(pairs: &[(String, String)]) -> String {
    let mut out = String::new();
    for (index, (name, value)) in pairs.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(name);
        out.push_str("=\"");
        escape_attribute(value, &mut out);
        out.push('"');
    }
    out
}

/** The pairs a processing instruction's data holds now. */
fn pairs_of(vm: &mut Vm, node: usize) -> Pairs {
    parse(&vm.page.text_content(node))
}

/** Write pairs back as the node's data, queueing one characterData record. */
fn store(vm: &mut Vm, node: usize, pairs: &Pairs) -> Res<()> {
    set_data(vm, node, &serialize(pairs))
}

/** Refuse a name the grammar cannot hold, as the DOM does for setAttribute and toggleAttribute. */
fn check_name(vm: &mut Vm, name: &str) -> Res<()> {
    if valid_name(name) {
        return Ok(());
    }
    let message = format!("'{name}' is not a valid attribute name.");
    Err(vm.dom_exception(INVALID_CHARACTER, &message))
}

/** Replace the value of a name already present, or append the name as the last pair. */
fn put(pairs: &mut Pairs, name: String, value: String) {
    match pairs.iter_mut().find(|(existing, _)| *existing == name) {
        Some(slot) => slot.1 = value,
        None => pairs.push((name, value)),
    }
}

/** ProcessingInstruction.getAttribute(name): the value, or null. */
pub(super) fn get_attribute(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    let name = vm.to_text(&call.arg(0))?;
    match pairs_of(vm, node).into_iter().find(|(key, _)| *key == name) {
        Some((_, value)) => JsValue::string(&value).map_err(Into::into),
        None => Ok(JsValue::Null),
    }
}

/** ProcessingInstruction.setAttribute(name, value): in place when present, appended otherwise.
Runs inside the reactions wrapper, which delivers the characterData record the write queues. */
pub(super) fn set_attribute(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| set_attribute_steps(vm, call))
}

/** The setAttribute steps, without the reactions wrapper. */
fn set_attribute_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    let name = vm.to_text(&call.arg(0))?;
    let value = vm.to_text(&call.arg(1))?;
    check_name(vm, &name)?;
    let mut pairs = pairs_of(vm, node);
    put(&mut pairs, name, value);
    store(vm, node, &pairs)?;
    Ok(JsValue::Undefined)
}

/** ProcessingInstruction.removeAttribute(name): drops the pair if present, writing nothing
when it is absent so no change record is queued. */
pub(super) fn remove_attribute(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| remove_attribute_steps(vm, call))
}

/** The removeAttribute steps, without the reactions wrapper. */
fn remove_attribute_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    let name = vm.to_text(&call.arg(0))?;
    let mut pairs = pairs_of(vm, node);
    let before = pairs.len();
    pairs.retain(|(key, _)| *key != name);
    if pairs.len() != before {
        store(vm, node, &pairs)?;
    }
    Ok(JsValue::Undefined)
}

/** ProcessingInstruction.hasAttribute(name). */
pub(super) fn has_attribute(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    let name = vm.to_text(&call.arg(0))?;
    let found = pairs_of(vm, node).iter().any(|(key, _)| *key == name);
    Ok(JsValue::Boolean(found))
}

/** ProcessingInstruction.hasAttributes(): whether the data holds any pair. */
pub(super) fn has_attributes(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    Ok(JsValue::Boolean(!pairs_of(vm, node).is_empty()))
}

/** ProcessingInstruction.getAttributeNames(): the names in data order. */
pub(super) fn get_attribute_names(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    let mut names: Vec<JsValue> = Vec::new();
    for (name, _) in pairs_of(vm, node) {
        names.push(JsValue::string(&name)?);
    }
    Ok(JsValue::Object(vm.new_array(names)?))
}

/** ProcessingInstruction.toggleAttribute(name, force): with no force, flips presence; a true
force adds an empty value when absent, a false one removes the pair when present. */
pub(super) fn toggle_attribute(vm: &mut Vm, call: Call) -> Res<JsValue> {
    components::reacting(vm, |vm| toggle_attribute_steps(vm, call))
}

/** The toggleAttribute steps, without the reactions wrapper. */
fn toggle_attribute_steps(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let node = dom::this_node(vm, &call)?;
    let name = vm.to_text(&call.arg(0))?;
    check_name(vm, &name)?;
    let mut pairs = pairs_of(vm, node);
    let present = pairs.iter().any(|(key, _)| *key == name);
    let forced = !matches!(call.arg(1), JsValue::Undefined);
    let wanted = if forced {
        call.arg(1).truthy()
    } else {
        !present
    };
    if wanted != present {
        if wanted {
            pairs.push((name, String::new()));
        } else {
            pairs.retain(|(key, _)| *key != name);
        }
        store(vm, node, &pairs)?;
    }
    Ok(JsValue::Boolean(wanted))
}
