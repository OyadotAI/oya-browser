/**
 * The JavaScript edge of Attr objects: the wrapper each Attr gets (one per record, so identity
 * holds), the Attr properties scripts read and write, the element methods that hand out or take
 * Attr objects, document.createAttribute(NS), and the NamedNodeMap `attributes` returns. Each entry
 * point checks its arguments and translates errors; the model lives in attr.rs.
 */
use super::attr::AttrNode;
use super::dom::{self, dom_error, name_error, native_name, optional_string_arg};
use super::namespaces::{self, NameKind, QName};
use crate::v9::value::JsValue;
use crate::v9::vm::data::{Class, HostData};
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::realm;
use crate::v9::vm::{Call, Res, Vm};

/** The nodeType an Attr reports: attributes are nodes of type 2 in the DOM standard. */
const ATTRIBUTE_NODE_TYPE: f64 = 2.0;
/** The DOM error a detach-or-remove of an attribute the element does not hold raises. */
const NOT_FOUND_ERROR: &str = "NotFoundError";
/** The DOM error an Attr already held by another element raises when it is attached again. */
const IN_USE_ERROR: &str = "InUseAttributeError";

/** The Attr id a value is, when the value is an Attr object. */
fn attr_id_of(vm: &Vm, value: &JsValue) -> Option<u64> {
    match super::host_data(vm, value)? {
        HostData::Attr(id) => Some(id),
        _ => None,
    }
}

/** The Attr id an argument names; anything else is a TypeError, as WebIDL words it. */
fn arg_attr(vm: &mut Vm, call: &Call, index: usize) -> Res<u64> {
    match attr_id_of(vm, &call.arg(index)) {
        Some(id) => Ok(id),
        None => Err(vm.type_error("parameter 1 is not of type 'Attr'.")),
    }
}

/** The JS object for an Attr, made once per record and reused, so an Attr keeps its identity. */
pub(in crate::v9) fn wrap_attr(vm: &mut Vm, id: u64) -> Res<JsValue> {
    if let Some(object) = vm.page.state.attr_wrappers.get(&id)
        && vm.heap.get(*object).is_some()
    {
        return Ok(JsValue::Object(*object));
    }
    let proto = realm().ids.attr_prototype;
    let object = vm.alloc(Object::new(Class::Host(Box::new(HostData::Attr(id))), Some(proto)))?;
    vm.page.state.attr_wrappers.insert(id, object);
    Ok(JsValue::Object(object))
}

/** A string the DOM hands to scripts. */
fn text(value: &str) -> Res<JsValue> {
    JsValue::string(value).map_err(Into::into)
}

/** The Attr properties and the node-shaped reads (nodeName, nodeValue, nodeType, textContent). */
pub(in crate::v9) fn attr_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(id) = attr_id_of(vm, &call.this) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let Some(record) = vm.page.attr_record(id).cloned() else {
        return Err(vm.type_error("Illegal invocation"));
    };
    match native_name(&call) {
        "name" | "nodeName" => text(&record.name),
        "localName" => text(&record.local),
        "namespaceURI" => record.namespace.as_deref().map_or(Ok(JsValue::Null), text),
        "prefix" => record.prefix.as_deref().map_or(Ok(JsValue::Null), text),
        "value" | "nodeValue" | "textContent" => text(&vm.page.attr_value_of(id)),
        "ownerElement" => match record.owner {
            Some(node) => dom::wrap(vm, node),
            None => Ok(JsValue::Null),
        },
        "specified" => Ok(JsValue::Boolean(true)),
        "nodeType" => Ok(JsValue::Number(ATTRIBUTE_NODE_TYPE)),
        _ => Ok(JsValue::Undefined),
    }
}

/** The value setters: writing an attached Attr writes its element's attribute, and writing a
detached one changes the value it keeps. */
pub(in crate::v9) fn attr_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(id) = attr_id_of(vm, &call.this) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let value = vm.to_text(&call.arg(0))?;
    let Some(record) = vm.page.attr_record(id).cloned() else {
        return Ok(JsValue::Undefined);
    };
    match record.owner {
        Some(node) => vm
            .page
            .set_named_attribute(node, &record.name, Some(&value))
            .map_err(|error| dom_error(vm, error))?,
        None => {
            if let Some(stored) = vm.page.state.attr_nodes.get_mut(&id) {
                stored.value = value;
            }
        }
    }
    Ok(JsValue::Undefined)
}

/** Writes an Attr's name and namespace into its element, so a namespaced one keeps its namespace. */
fn write_record(vm: &mut Vm, node: usize, record: &AttrNode, value: &str) -> Res<()> {
    let result = match record.namespace.as_deref() {
        Some(namespace) => {
            let qname = QName {
                prefix: record.prefix.clone(),
                local: record.local.clone(),
            };
            vm.page
                .set_namespaced_attribute(node, Some(namespace), &qname, &record.name, value)
        }
        None => vm
            .page
            .set_named_attribute(node, &record.name, Some(value)),
    };
    result.map_err(|error| dom_error(vm, error))
}

/** getAttributeNode and getAttributeNodeNS: the element's Attr for the name, or null when it has none. */
fn attr_for(vm: &mut Vm, node: usize, name: Option<String>) -> Res<JsValue> {
    let Some(name) = name else {
        return Ok(JsValue::Null);
    };
    if vm.page.attribute_value(node, &name).is_none() {
        return Ok(JsValue::Null);
    }
    let id = vm.page.attr_identity(node, &name);
    wrap_attr(vm, id)
}

/** setAttributeNode and setAttributeNodeNS: attach an Attr, returning the Attr it replaced (or null).
An Attr already held by another element cannot be attached, as the DOM standard requires. */
fn set_attr_node(vm: &mut Vm, node: usize, call: &Call) -> Res<JsValue> {
    let id = arg_attr(vm, call, 0)?;
    let Some(record) = vm.page.attr_record(id).cloned() else {
        return Err(vm.type_error("parameter 1 is not of type 'Attr'."));
    };
    if record.owner.is_some_and(|other| other != node) {
        return Err(vm.dom_exception(IN_USE_ERROR, "The attribute is in use by another element."));
    }
    if record.owner == Some(node) {
        return wrap_attr(vm, id);
    }
    let replaced = match vm.page.attribute_value(node, &record.name) {
        Some(previous) => {
            let old = vm.page.attr_identity(node, &record.name);
            vm.page.detach_attr(node, &record.name, Some(previous));
            Some(old)
        }
        None => None,
    };
    let value = vm.page.attr_value_of(id);
    write_record(vm, node, &record, &value)?;
    vm.page.attach_attr(id, node, &record.name);
    match replaced {
        Some(old) => wrap_attr(vm, old),
        None => Ok(JsValue::Null),
    }
}

/** removeAttributeNode: remove the element's attribute the Attr names, returning that Attr, which
keeps the value it had. An Attr the element does not hold raises NotFoundError. */
fn remove_attr_node(vm: &mut Vm, node: usize, call: &Call) -> Res<JsValue> {
    let id = arg_attr(vm, call, 0)?;
    let owner = vm.page.attr_record(id).and_then(|record| record.owner);
    if owner != Some(node) {
        return Err(vm.dom_exception(NOT_FOUND_ERROR, "The attribute is not on this element."));
    }
    let name = vm.page.attr_record(id).map(|record| record.name.clone());
    if let Some(name) = name {
        vm.page
            .set_named_attribute(node, &name, None)
            .map_err(|error| dom_error(vm, error))?;
    }
    wrap_attr(vm, id)
}

/** The element methods that hand out, attach or remove Attr objects. */
pub(in crate::v9) fn element_attr_method(vm: &mut Vm, node: usize, call: &Call) -> Res<JsValue> {
    match native_name(call) {
        "getAttributeNode" => {
            let name = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
            attr_for(vm, node, Some(name))
        }
        "getAttributeNodeNS" => {
            let found = namespaced_name(vm, node, call)?;
            attr_for(vm, node, found)
        }
        "setAttributeNode" | "setAttributeNodeNS" => set_attr_node(vm, node, call),
        "removeAttributeNode" => remove_attr_node(vm, node, call),
        _ => Ok(JsValue::Undefined),
    }
}

/** The stored name a namespace-aware read names, or None when the element holds no such attribute. */
fn namespaced_name(vm: &mut Vm, node: usize, call: &Call) -> Res<Option<String>> {
    let namespace = optional_string_arg(vm, call, 0)?;
    let local = vm.to_text(&call.arg(1))?;
    Ok(vm
        .page
        .namespaced_attribute_name(node, namespace.as_deref(), &local))
}

/** document.createAttribute and createAttributeNS: a detached Attr with no value yet. The name is
validated as an attribute name, and the NS form also splits its prefix and checks the pairing. */
pub(in crate::v9) fn create_attribute(vm: &mut Vm, call: &Call, namespaced: bool) -> Res<JsValue> {
    let (namespace, qualified, qname) = if namespaced {
        let namespace = optional_string_arg(vm, call, 0)?;
        let qualified = vm.to_text(&call.arg(1))?;
        let qname = namespaces::validate_and_extract(namespace.as_deref(), &qualified, NameKind::Attribute)
            .map_err(|error| name_error(vm, error))?;
        (namespace, qualified, qname)
    } else {
        let qualified = vm.to_text(&call.arg(0))?.to_ascii_lowercase();
        namespaces::validate_local_name(&qualified, NameKind::Attribute)
            .map_err(|error| name_error(vm, error))?;
        let qname = QName {
            prefix: None,
            local: qualified.clone(),
        };
        (None, qualified, qname)
    };
    let record = AttrNode {
        owner: None,
        name: qualified,
        namespace,
        prefix: qname.prefix,
        local: qname.local,
        value: String::new(),
    };
    let id = vm.page.new_attr(record);
    wrap_attr(vm, id)
}

/** The NamedNodeMap `attributes` returns: an array of the element's Attr objects, with the map's
lookups on its prototype and each Attr reachable by name. */
pub(in crate::v9) fn named_node_map(vm: &mut Vm, node: usize) -> Res<JsValue> {
    let names: Vec<String> = vm
        .page
        .view(node)
        .map(|view| view.attributes.keys().cloned().collect())
        .unwrap_or_default();
    let list = vm.new_array(Vec::new())?;
    let mut named = Vec::with_capacity(names.len());
    for name in names {
        let id = vm.page.attr_identity(node, &name);
        let item = wrap_attr(vm, id)?;
        vm.array_append(&JsValue::Object(list), item.clone())?;
        if let JsValue::Object(object) = item {
            named.push((name, object));
        }
    }
    vm.set_proto(list, Some(realm().ids.named_node_map_prototype))?;
    dom::named_attributes(vm, list, named)?;
    Ok(JsValue::Object(list))
}
