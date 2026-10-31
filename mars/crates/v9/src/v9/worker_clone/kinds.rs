/**
 * The structured-clone kinds beyond plain data: Date, RegExp, Map, Set, primitive wrappers,
 * errors and Blobs/Files, as HTML's StructuredSerialize lists them. Encoding reads the native
 * class (never author-replaceable prototypes); decoding allocates with the receiving realm's
 * intrinsic prototypes and fills Map and Set tables directly, so no script runs while a message
 * is revived.
 */
use super::*;
use crate::v9::vm::Thrown;

/** The error constructors a clone may name; any other name revives as Error. */
const ERROR_NAMES: [&str; 7] = [
    "Error",
    "EvalError",
    "RangeError",
    "ReferenceError",
    "SyntaxError",
    "TypeError",
    "URIError",
];

/** A node's kind and its own fields. */
pub(super) type Encoded = (Kind, Vec<(JsString, Atom)>);

/** Encode a referenced child value. */
pub(super) type Nested<'a> = dyn FnMut(&mut Vm, &JsValue, &mut WorkerData) -> Res<Atom> + 'a;

/** The node for a value of one of these kinds, or None when the value is plain data (or refused later). */
pub(super) fn encode(
    vm: &mut Vm,
    value: &JsValue,
    class: &Class,
    graph: &mut WorkerData,
    nested: &mut Nested,
) -> Res<Option<Encoded>> {
    if let Some(blob) = blob(vm, value)? {
        return Ok(Some((blob, Vec::new())));
    }
    let id = value.object().expect("encoded objects are objects");
    if let Some(text) = vm.string_data(id) {
        return Ok(Some((Kind::Wrapper(Primitive::String(text)), Vec::new())));
    }
    let kind = match class {
        Class::Date(time) => Kind::Date(time.to_bits()),
        Class::RegExp(data) => Kind::RegExp(data.source.clone(), data.flags.clone()),
        Class::Boolean(flag) => Kind::Wrapper(Primitive::Boolean(*flag)),
        Class::Number(number) => Kind::Wrapper(Primitive::Number(number.to_bits())),
        Class::BigInt(_) | Class::BigIntWide(_) => bigint(vm, class)?,
        Class::Map(table) => Kind::Map(entries(vm, table, graph, nested)?),
        Class::Set(table) => Kind::Set(
            entries(vm, table, graph, nested)?
                .into_iter()
                .map(|(key, _)| key)
                .collect(),
        ),
        Class::Error => return error(vm, value).map(Some),
        _ => return Ok(None),
    };
    Ok(Some((kind, Vec::new())))
}

/** A Blob or File node, refused when its bytes alone exceed a message. */
fn blob(vm: &mut Vm, value: &JsValue) -> Res<Option<Kind>> {
    let Some((kind, bytes, file)) = crate::v9::platform::blob::blob_parts(vm, value) else {
        return Ok(None);
    };
    if bytes.len() > WORKER_MESSAGE_BYTES {
        return Err(super::encode::refused(vm));
    }
    let text = |value: JsValue| match value {
        JsValue::String(text) => text,
        _ => JsString::empty(),
    };
    let file = file.map(|(name, modified)| {
        let time = modified.number_value().unwrap_or_default();
        (text(name), time.to_bits())
    });
    Ok(Some(Kind::Blob(bytes, text(kind), file)))
}

/** A BigInt object's primitive. */
fn bigint(vm: &mut Vm, class: &Class) -> Res<Kind> {
    let value = match class {
        Class::BigInt(value) => JsValue::BigInt(*value),
        Class::BigIntWide(value) => JsValue::BigIntWide(value.clone()),
        _ => return Err(super::encode::refused(vm)),
    };
    let big = value.bigint_value().expect("BigInt value");
    Ok(Kind::Wrapper(Primitive::BigInt(big)))
}

/** Map or Set entries, each side encoded in insertion order. */
fn entries(
    vm: &mut Vm,
    table: &crate::v9::vm::data::Table,
    graph: &mut WorkerData,
    nested: &mut Nested,
) -> Res<Vec<(Atom, Atom)>> {
    let pairs: Vec<(JsValue, JsValue)> = table.entries.iter().flatten().cloned().collect();
    let mut encoded = Vec::with_capacity(pairs.len());
    for (key, value) in pairs {
        let key = nested(vm, &key, graph)?;
        let value = nested(vm, &value, graph)?;
        encoded.push((key, value));
    }
    Ok(encoded)
}

/** An error node: its name picks the prototype; an own message travels as the one field. */
fn error(vm: &mut Vm, value: &JsValue) -> Res<Encoded> {
    let name = vm.get(value, &Key::from("name"))?;
    let name = vm.to_string(&name)?.lossy();
    let name = if ERROR_NAMES.contains(&name.as_str()) {
        name
    } else {
        "Error".into()
    };
    let id = value.object().expect("errors are objects");
    let mut fields = Vec::new();
    if vm.own(id, &Key::from("message"))?.is_some() {
        let message = vm.get(value, &Key::from("message"))?;
        let message = vm.to_string(&message)?;
        fields.push((
            JsString::from_utf8("message")?,
            Atom::Value(Primitive::String(message)),
        ));
    }
    Ok((Kind::Error(name), fields))
}

/** Allocate the object for one of these kinds, or None for the original plain kinds. */
pub(super) fn allocate(vm: &mut Vm, kind: &Kind) -> Res<Option<ObjectId>> {
    let ids = &realm().ids;
    let object = |class: Class, proto: ObjectId| Object::new(class, Some(proto));
    let id = match kind {
        Kind::Date(bits) => vm.alloc(object(
            Class::Date(f64::from_bits(*bits)),
            ids.date_prototype,
        ))?,
        Kind::RegExp(source, flags) => vm.new_regexp(source.clone(), flags).map_err(invalid)?,
        Kind::Map(_) => vm.alloc(object(Class::Map(Box::default()), ids.map_prototype))?,
        Kind::Set(_) => vm.alloc(object(Class::Set(Box::default()), ids.set_prototype))?,
        Kind::Wrapper(primitive) => vm.to_object(&resolve(&Atom::Value(primitive.clone()), &[]))?,
        Kind::Error(name) => vm.alloc(object(Class::Error, error_prototype(name)))?,
        Kind::Blob(bytes, kind, file) => restore_blob(vm, bytes, kind, file)?,
        _ => return Ok(None),
    };
    Ok(Some(id))
}

/** A RegExp that does not compile here is a forged message, not a script error. */
fn invalid(_: Thrown) -> Thrown {
    EngineError::invalid("Invalid worker RegExp.").into()
}

/** The intrinsic prototype for an error name. */
fn error_prototype(name: &str) -> ObjectId {
    let ids = &realm().ids;
    match name {
        "EvalError" => ids.eval_error_prototype,
        "RangeError" => ids.range_error_prototype,
        "ReferenceError" => ids.reference_error_prototype,
        "SyntaxError" => ids.syntax_error_prototype,
        "TypeError" => ids.type_error_prototype,
        "URIError" => ids.uri_error_prototype,
        _ => ids.error_prototype,
    }
}

/** A Blob or File over the message's bytes. */
fn restore_blob(
    vm: &mut Vm,
    bytes: &[u8],
    kind: &JsString,
    file: &Option<(JsString, u64)>,
) -> Res<ObjectId> {
    let file = file.as_ref().map(|(name, bits)| {
        (
            JsValue::String(name.clone()),
            JsValue::Number(f64::from_bits(*bits)),
        )
    });
    crate::v9::platform::blob::restore_blob(vm, bytes.to_vec(), JsValue::String(kind.clone()), file)
}

/** Fill a revived Map or Set from its resolved entries. */
pub(super) fn wire(vm: &mut Vm, id: ObjectId, kind: &Kind, objects: &[ObjectId]) {
    let pairs: Vec<(JsValue, JsValue)> = match kind {
        Kind::Map(entries) => entries
            .iter()
            .map(|(key, value)| (resolve(key, objects), resolve(value, objects)))
            .collect(),
        Kind::Set(values) => values
            .iter()
            .map(|value| (resolve(value, objects), JsValue::Undefined))
            .collect(),
        _ => return,
    };
    for (key, value) in pairs {
        vm.heap.track_value(&key);
        vm.heap.track_value(&value);
        let units = (key.string_units() + value.string_units()) as isize;
        if let Some(Class::Map(table) | Class::Set(table)) =
            vm.heap.get_mut(id).map(|object| &mut object.class)
            && table.set(key, value)
        {
            vm.heap.adjust(1, units);
        }
    }
}

/** A message atom as a value of this packet. */
fn resolve(value: &Atom, objects: &[ObjectId]) -> JsValue {
    super::decode::resolve(value, objects)
}
