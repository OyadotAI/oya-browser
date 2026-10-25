/**
 * FormData, after Blink's core/html/forms/form_data.cc and the XHR spec: an ordered list of
 * (name, value) entries where a value is a USVString or a File. A Blob appended is wrapped in a
 * File named `blob` (or the filename given), as "create an entry" says. The entries live in a
 * hidden array of [name, value] pairs on the wrapper.
 *
 * ponytail: `new FormData(form)` does not yet build the form's entry list (it starts empty), and
 * the iterators walk a snapshot taken when they are made rather than the live list.
 */
use super::blob;
use crate::v9::value::{JsString, JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{CONFIGURABLE, HIDDEN_ATTRS, Key, Object, Prop, Slot};
use crate::v9::vm::realm::{Builder, Static, realm, string, symbol};
use crate::v9::vm::{Call, Res, Vm};
use std::sync::OnceLock;

/** The FormData prototype, set once at install. */
static PROTO: OnceLock<u32> = OnceLock::new();

/** Install FormData. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.method(proto, "append", 2, append);
    b.method(proto, "set", 2, set);
    b.method(proto, "delete", 1, delete);
    b.method(proto, "get", 1, get);
    b.method(proto, "getAll", 1, get_all);
    b.method(proto, "has", 1, has);
    b.method(proto, "forEach", 1, for_each);
    b.method(proto, "keys", 0, keys);
    b.method(proto, "values", 0, values);
    let entries = b.method(proto, "entries", 0, entries);
    b.put(
        proto,
        Key::Sym(symbol::ITERATOR),
        Static::Value(JsValue::Object(entries)),
        HIDDEN_ATTRS,
    );
    b.put(
        proto,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("FormData")),
        CONFIGURABLE,
    );
    let ctor = b.constructor("FormData", 0, construct, proto, None);
    b.global("FormData", ctor);
    let _ = PROTO.set(proto);
}

/** The private key of the entry list. */
fn list_key() -> Key {
    Key::private("formEntries")
}

/** One entry: its name and its value (a string or a File). */
type Entry = (JsString, JsValue);

/** `new FormData()`. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(vm.type_error("Failed to construct 'FormData': Please use the 'new' operator, this DOM object constructor cannot be called as a function."));
    };
    let _ = realm();
    let proto = vm.prototype_for(&target, *PROTO.get().expect("installed"))?;
    if !matches!(call.arg(0), JsValue::Undefined) && super::dom::node_of(vm, &call.arg(0)).is_none()
    {
        return Err(vm.type_error(
            "Failed to construct 'FormData': parameter 1 is not of type 'HTMLFormElement'.",
        ));
    }
    let id = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    store(vm, id, &[])?;
    Ok(JsValue::Object(id))
}

/** The receiver's entries, or the TypeError for a receiver that is no FormData. */
fn load(vm: &mut Vm, call: &Call) -> Res<(ObjectId, Vec<Entry>)> {
    let found =
        call.this
            .object()
            .and_then(|id| match &vm.heap.get(id)?.props.get(&list_key())?.slot {
                Slot::Data(JsValue::Object(list)) => Some((id, *list)),
                _ => None,
            });
    let Some((id, list)) = found else {
        return Err(vm.type_error("Illegal invocation: the receiver is not a FormData"));
    };
    let mut out = Vec::new();
    for pair in vm.iterate_to_vec(&JsValue::Object(list))? {
        let name = vm.get(&pair, &Key::from("0"))?;
        let value = vm.get(&pair, &Key::from("1"))?;
        if let JsValue::String(name) = name {
            out.push((name, value));
        }
    }
    Ok((id, out))
}

/** Replace the receiver's entries. */
fn store(vm: &mut Vm, id: ObjectId, entries: &[Entry]) -> Res<()> {
    let mut pairs = Vec::with_capacity(entries.len());
    for (name, value) in entries {
        let pair = vm.new_array(vec![JsValue::String(name.clone()), value.clone()])?;
        pairs.push(JsValue::Object(pair));
    }
    let list = vm.new_array(pairs)?;
    let prop = Prop {
        slot: Slot::Data(JsValue::Object(list)),
        attrs: HIDDEN_ATTRS,
    };
    vm.heap.put(id, list_key(), prop);
    Ok(())
}

/** The TypeError for too few arguments. */
fn arity(vm: &mut Vm, call: &Call, member: &str, needed: usize) -> Res<()> {
    if call.args.len() >= needed {
        return Ok(());
    }
    let message = format!(
        "Failed to execute '{member}' on 'FormData': {needed} arguments required, but only {} present.",
        call.args.len()
    );
    Err(vm.type_error(&message))
}

/** A USVString argument. */
fn usv(vm: &mut Vm, value: &JsValue) -> Res<JsString> {
    let text = vm.to_string(value)?.lossy();
    Ok(JsString::from_utf8(&text)?)
}

/** "Create an entry": a string value is a USVString; a Blob becomes a File named after the
filename argument, the File's own name, or `blob`, in that order. */
fn create_entry(vm: &mut Vm, call: &Call, member: &str) -> Res<Entry> {
    arity(vm, call, member, 2)?;
    let name = usv(vm, &call.arg(0))?;
    let value = call.arg(1);
    let Some(bytes) = blob::blob_bytes(vm, &value) else {
        if call.args.len() > 2 {
            let message = format!(
                "Failed to execute '{member}' on 'FormData': parameter 2 is not of type 'Blob'."
            );
            return Err(vm.type_error(&message));
        }
        return Ok((name, JsValue::String(usv(vm, &value)?)));
    };
    let filename = match call.arg(2) {
        JsValue::Undefined if blob::is_file(vm, &value) => return Ok((name, value)),
        JsValue::Undefined => String::from("blob"),
        given => usv(vm, &given)?.lossy(),
    };
    let kind = vm.get(&value, &Key::from("type"))?;
    let kind = vm.to_text(&kind)?;
    let file = blob::new_file(vm, bytes, &kind, &filename)?;
    Ok((name, JsValue::Object(file)))
}

/** `append(name, value, filename)`. */
fn append(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, mut entries) = load(vm, &call)?;
    entries.push(create_entry(vm, &call, "append")?);
    store(vm, id, &entries)?;
    Ok(JsValue::Undefined)
}

/** `set(name, value, filename)`: replace the first entry of that name, drop the others. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, entries) = load(vm, &call)?;
    let entry = create_entry(vm, &call, "set")?;
    let mut out = Vec::with_capacity(entries.len() + 1);
    let mut placed = false;
    for existing in entries {
        if existing.0 != entry.0 {
            out.push(existing);
        } else if !placed {
            out.push(entry.clone());
            placed = true;
        }
    }
    if !placed {
        out.push(entry);
    }
    store(vm, id, &out)?;
    Ok(JsValue::Undefined)
}

/** The name argument of a lookup. */
fn wanted(vm: &mut Vm, call: &Call, member: &str) -> Res<JsString> {
    arity(vm, call, member, 1)?;
    usv(vm, &call.arg(0))
}

/** `delete(name)`. */
fn delete(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, mut entries) = load(vm, &call)?;
    let name = wanted(vm, &call, "delete")?;
    entries.retain(|(entry, _)| *entry != name);
    store(vm, id, &entries)?;
    Ok(JsValue::Undefined)
}

/** `get(name)`: the first value, or null. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, entries) = load(vm, &call)?;
    let name = wanted(vm, &call, "get")?;
    let found = entries.into_iter().find(|(entry, _)| *entry == name);
    Ok(found.map_or(JsValue::Null, |(_, value)| value))
}

/** `getAll(name)`: every value of that name, in order. */
fn get_all(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, entries) = load(vm, &call)?;
    let name = wanted(vm, &call, "getAll")?;
    let values = entries
        .into_iter()
        .filter(|(entry, _)| *entry == name)
        .map(|(_, value)| value)
        .collect();
    Ok(JsValue::Object(vm.new_array(values)?))
}

/** `has(name)`. */
fn has(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, entries) = load(vm, &call)?;
    let name = wanted(vm, &call, "has")?;
    Ok(JsValue::Boolean(
        entries.iter().any(|(entry, _)| *entry == name),
    ))
}

/** `forEach(callback, thisArg)`: callback(value, name, formData) per entry. */
fn for_each(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (_, entries) = load(vm, &call)?;
    let callback = call.arg(0);
    if !vm.is_callable(&callback) {
        return Err(vm.type_error("Failed to execute 'forEach' on 'FormData': The callback provided as parameter 1 is not a function."));
    }
    for (name, value) in entries {
        let args = vec![value, JsValue::String(name), call.this.clone()];
        vm.call(callback.clone(), call.arg(1), args)?;
    }
    Ok(JsValue::Undefined)
}

/** An array iterator over a snapshot of the entries, mapped by `pick`. */
fn iterate(vm: &mut Vm, call: &Call, pick: fn(&mut Vm, Entry) -> Res<JsValue>) -> Res<JsValue> {
    let (_, entries) = load(vm, call)?;
    let mut items = Vec::with_capacity(entries.len());
    for entry in entries {
        items.push(pick(vm, entry)?);
    }
    let array = JsValue::Object(vm.new_array(items)?);
    let method = vm.get(&array, &Key::Sym(symbol::ITERATOR))?;
    vm.call(method, array, Vec::new())
}

/** `keys()`. */
fn keys(vm: &mut Vm, call: Call) -> Res<JsValue> {
    iterate(vm, &call, |_, (name, _)| Ok(JsValue::String(name)))
}

/** `values()`. */
fn values(vm: &mut Vm, call: Call) -> Res<JsValue> {
    iterate(vm, &call, |_, (_, value)| Ok(value))
}

/** `entries()` and `[Symbol.iterator]()`: [name, value] pairs. */
fn entries(vm: &mut Vm, call: Call) -> Res<JsValue> {
    iterate(vm, &call, |vm, (name, value)| {
        Ok(JsValue::Object(
            vm.new_array(vec![JsValue::String(name), value])?,
        ))
    })
}
