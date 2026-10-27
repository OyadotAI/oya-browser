/** Array: the constructor, statics and every ES2023 prototype method, generic over array-likes. */
use super::*;
use crate::protocol::EngineError;
use crate::v9::vm::data::{Class, IterKind, IterState};
use crate::v9::vm::realm::{NativeFn, symbol};

mod concat;
mod copy;
mod create;
mod flatten;
mod from_async;
mod join_sparse;
mod sort_sparse;
mod species;
mod unscopables;

/** Array and Array.prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.array_prototype = proto;
    b.value(
        proto,
        "length",
        JsValue::Number(0.0),
        crate::v9::vm::heap::WRITABLE,
    );
    let ctor = b.constructor("Array", 1, construct, proto, None);
    b.ids.array = ctor;
    b.global("Array", ctor);
    b.method(ctor, "isArray", 1, is_array);
    b.method(ctor, "of", 0, of);
    b.method(ctor, "from", 1, from);
    from_async::install(b, ctor);
    super::typed::species_getter(b, ctor);
    let methods: [(&'static str, u32, NativeFn); 38] = [
        ("at", 1, at),
        ("concat", 1, concat),
        ("copyWithin", 2, copy_within),
        ("entries", 0, entries),
        ("every", 1, every),
        ("fill", 1, fill),
        ("filter", 1, filter),
        ("find", 1, find),
        ("findIndex", 1, find_index),
        ("findLast", 1, find_last),
        ("findLastIndex", 1, find_last_index),
        ("flat", 0, flat),
        ("flatMap", 1, flat_map),
        ("forEach", 1, for_each),
        ("includes", 1, includes),
        ("indexOf", 1, index_of),
        ("join", 1, join),
        ("keys", 0, keys),
        ("lastIndexOf", 1, last_index_of),
        ("map", 1, map),
        ("pop", 0, pop),
        ("push", 1, push),
        ("reduce", 1, reduce),
        ("reduceRight", 1, reduce_right),
        ("reverse", 0, reverse),
        ("shift", 0, shift),
        ("slice", 2, slice),
        ("some", 1, some),
        ("sort", 1, sort),
        ("splice", 2, splice),
        ("toLocaleString", 0, to_locale_string),
        ("toReversed", 0, to_reversed),
        ("toSorted", 1, to_sorted),
        ("toSpliced", 2, to_spliced),
        ("toString", 0, to_string),
        ("unshift", 1, unshift),
        ("with", 2, with),
        ("group", 1, unsupported_group),
    ];
    for (name, length, function) in methods {
        if name == "group" {
            continue;
        }
        b.method(proto, name, length, function);
    }
    // Typed arrays share the generic methods that never change an array's length.
    let typed: Vec<(&'static str, u32, NativeFn)> = methods
        .into_iter()
        .filter(|(name, ..)| TYPED_SHARED.contains(name))
        .chain([
            ("values", 0, values as NativeFn),
            ("keys", 0, keys),
            ("entries", 0, entries),
        ])
        .collect();
    super::typed::install(b, &typed);
    let values = b.method(proto, "values", 0, values);
    b.ids.array_values = values;
    b.put(
        proto,
        Key::Sym(symbol::ITERATOR),
        crate::v9::vm::realm::Static::Value(JsValue::Object(values)),
        crate::v9::vm::heap::HIDDEN_ATTRS,
    );
    unscopables::install(b, proto);
}

/** Array.prototype methods typed arrays reuse: generic, and length-preserving. */
const TYPED_SHARED: [&str; 18] = [
    "at",
    "copyWithin",
    "every",
    "fill",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "forEach",
    "includes",
    "indexOf",
    "join",
    "lastIndexOf",
    "reduce",
    "reduceRight",
    "reverse",
    "some",
    "toString",
];

/** Placeholder kept out of the table above. */
fn unsupported_group(vm: &mut Vm, _: Call) -> Res<JsValue> {
    Err(vm.unsupported_error("Array.prototype.group"))
}

/** `Array(n)` and `Array(a, b, ...)`. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = if let [JsValue::Number(length)] = call.args {
        if *length < 0.0 || length.fract() != 0.0 || *length > f64::from(u32::MAX) {
            return Err(vm.range_error("Invalid array length"));
        }
        vm.new_array_length(*length as usize)?
    } else {
        vm.new_array(call.args.to_vec())?
    };
    if let Some(target) = &call.new_target {
        let proto = vm.prototype_for(target, crate::v9::vm::realm::realm().ids.array_prototype)?;
        vm.set_proto(id, Some(proto))?;
    }
    Ok(JsValue::Object(id))
}

/** Whether a value has direct array storage, for storage optimizations without proxy traps. */
pub(in crate::v9) fn is_array_value(vm: &Vm, value: &JsValue) -> bool {
    matches!(value, JsValue::Object(id) if matches!(vm.heap.get(*id).map(|o| &o.class), Some(Class::Array(_))))
}

/** Array.isArray. */
fn is_array(vm: &mut Vm, call: Call) -> Res<JsValue> {
    is_array_checked(vm, &call.arg(0)).map(JsValue::Boolean)
}

/** IsArray follows proxy targets without calling traps; a revoked proxy is a TypeError. Each
hop consumes work so a forged cycle cannot loop forever or exhaust the native stack. */
pub(in crate::v9) fn is_array_checked(vm: &mut Vm, value: &JsValue) -> Res<bool> {
    is_array_for(vm, value, "IsArray")
}

/** The calling operation supplies diagnostics while target traversal preserves identical IsArray semantics. */
pub(in crate::v9) fn is_array_for(vm: &mut Vm, value: &JsValue, operation: &str) -> Res<bool> {
    let mut value = value.clone();
    let mut depth = 0;
    loop {
        let JsValue::Object(id) = value else {
            return Ok(false);
        };
        if is_array_value(vm, &value) || id == vm.object_intrinsics(id).ids.array_prototype {
            return Ok(true);
        }
        match vm.heap.get(id).map(|object| &object.class) {
            Some(Class::Array(_)) => return Ok(true),
            Some(Class::Proxy(proxy)) => {
                // V8 walks a proxy chain natively here, so the chain keeps the native bound.
                if depth >= crate::constants::VM_NESTING_DEPTH {
                    return Err(vm.range_error("Maximum call stack size exceeded"));
                }
                depth += 1;
                let Some(target) = proxy.target else {
                    return Err(vm.type_error(&format!(
                        "Cannot perform '{operation}' on a proxy that has been revoked"
                    )));
                };
                value = JsValue::Object(target);
                vm.charge(1)?;
            }
            _ => return Ok(false),
        }
    }
}

/** Array.of. */
fn of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    create::of(vm, call)
}

/** Array.from observes each source step and mapper before requesting the following value. */
fn from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    create::from(vm, call)
}

/** Refuse lengths beyond the array bound before allocating. */
fn check_length(vm: &mut Vm, length: usize) -> Res<()> {
    if length > vm.limits.array_length {
        return Err(EngineError::limit("V9 array length", vm.limits.array_length).into());
    }
    Ok(())
}

/** The index key. */
fn index_key(index: usize) -> Res<Key> {
    Ok(Key::Str(JsString::from_utf8(&index.to_string())?))
}

/** Element `index` of an array-like (fast for arrays). */
pub(in crate::v9) fn get_index(vm: &mut Vm, object: &JsValue, index: usize) -> Res<JsValue> {
    if let JsValue::Object(id) = object
        && let Some(Class::Array(elements)) = vm.heap.get(*id).map(|o| &o.class)
        && let Some(value) = elements.get(index)
        && *value != JsValue::Hole
    {
        return Ok(value.clone());
    }
    vm.get(object, &index_key(index)?)
}

/** Whether an array-like has element `index`. */
fn has_index(vm: &mut Vm, object: &JsValue, index: usize) -> Res<bool> {
    let JsValue::Object(id) = object else {
        return Ok(false);
    };
    // A hole walks the chain: Array.prototype or a property-list element may answer.
    if let Some(Class::Array(elements)) = vm.heap.get(*id).map(|o| &o.class)
        && elements
            .get(index)
            .is_some_and(|value| *value != JsValue::Hole)
    {
        return Ok(true);
    }
    vm.has_property(*id, &index_key(index)?)
}

/** Store element `index`. */
fn set_index(vm: &mut Vm, object: &JsValue, index: usize, value: JsValue) -> Res<()> {
    vm.set(object, index_key(index)?, value, true)
}

/** Delete element `index`. */
fn delete_index(vm: &mut Vm, object: &JsValue, index: usize) -> Res<()> {
    if let JsValue::Object(id) = object
        && !vm.delete(*id, &index_key(index)?)?
    {
        return Err(vm.type_error("Cannot delete array element"));
    }
    Ok(())
}

/** Set `length`. */
fn set_length(vm: &mut Vm, object: &JsValue, length: usize) -> Res<()> {
    vm.set(object, key("length"), JsValue::Number(length as f64), true)
}

/** `this` as an object and its length. */
fn receiver(vm: &mut Vm, call: &Call, name: &str) -> Res<(JsValue, usize)> {
    let id = this_object(vm, call, name)?;
    let object = JsValue::Object(id);
    let length = vm.length_of(&object)?;
    Ok((object, length))
}

/** The callback argument, checked. */
fn callback(vm: &mut Vm, call: &Call, name: &str) -> Res<JsValue> {
    let function = call.arg(0);
    if !vm.is_callable(&function) {
        return Err(vm.type_error(&format!("{name}: callback is not a function")));
    }
    Ok(function)
}

/** Array.prototype.at. */
fn at(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.at")?;
    let relative = vm.to_integer(&call.arg(0))?;
    let index = if relative < 0.0 {
        length as f64 + relative
    } else {
        relative
    };
    if index < 0.0 || index >= length as f64 {
        return Ok(JsValue::Undefined);
    }
    get_index(vm, &object, index as usize)
}

/** Whether concat spreads a value. */
fn spreadable(vm: &mut Vm, value: &JsValue) -> Res<bool> {
    if !matches!(value, JsValue::Object(_)) {
        return Ok(false);
    }
    let flag = vm.get(value, &Key::Sym(symbol::IS_CONCAT_SPREADABLE))?;
    if flag != JsValue::Undefined {
        return Ok(flag.truthy());
    }
    is_array_checked(vm, value)
}

/** Array.prototype.concat. */
fn concat(vm: &mut Vm, call: Call) -> Res<JsValue> {
    concat::run(vm, call)
}

/** Array.prototype.copyWithin. */
fn copy_within(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.copyWithin")?;
    let target = vm.relative_index(&call.arg(0), length, 0)?;
    let start = vm.relative_index(&call.arg(1), length, 0)?;
    let end = vm.relative_index(&call.arg(2), length, length)?;
    let count = end.saturating_sub(start).min(length - target);
    let backwards = start < target && target < start + count;
    for offset in 0..count {
        let offset = if backwards {
            count - offset - 1
        } else {
            offset
        };
        if has_index(vm, &object, start + offset)? {
            let value = get_index(vm, &object, start + offset)?;
            set_index(vm, &object, target + offset, value)?;
        } else {
            delete_index(vm, &object, target + offset)?;
        }
        vm.charge(1)?;
    }
    Ok(object)
}

/** A native array iterator. */
fn iterator(vm: &mut Vm, call: &Call, kind: IterKind) -> Res<JsValue> {
    let id = this_object(vm, call, "Array iterator")?;
    vm.new_iterator(IterState::Array {
        object: JsValue::Object(id),
        index: 0,
        kind,
        done: false,
    })
    .map(JsValue::Object)
}

/** Array.prototype.entries. */
fn entries(vm: &mut Vm, call: Call) -> Res<JsValue> {
    iterator(vm, &call, IterKind::Entries)
}

/** Array.prototype.keys. */
fn keys(vm: &mut Vm, call: Call) -> Res<JsValue> {
    iterator(vm, &call, IterKind::Keys)
}

/** Array.prototype.values and [Symbol.iterator]. */
fn values(vm: &mut Vm, call: Call) -> Res<JsValue> {
    iterator(vm, &call, IterKind::Values)
}

/** How an iteration method uses its callback results. */
#[derive(Clone, Copy, PartialEq, Eq)]
enum Walk {
    Every,
    Some,
    ForEach,
    Map,
    Filter,
    Find,
    FindIndex,
}

/** Shared loop for the callback methods. */
fn walk(vm: &mut Vm, call: Call, name: &str, mode: Walk, reverse: bool) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, name)?;
    let function = callback(vm, &call, name)?;
    let this_arg = call.arg(1);
    let result = match mode {
        Walk::Map => Some(species::create(vm, &object, length)?),
        Walk::Filter => Some(species::create(vm, &object, 0)?),
        _ => None,
    };
    let skips_holes = !matches!(mode, Walk::Find | Walk::FindIndex);
    let mut filtered = 0;
    for step in 0..length {
        vm.charge(1)?;
        let index = if reverse { length - 1 - step } else { step };
        if skips_holes && !has_index(vm, &object, index)? {
            continue;
        }
        let value = get_index(vm, &object, index)?;
        let outcome = vm.call(
            function.clone(),
            this_arg.clone(),
            vec![value.clone(), JsValue::Number(index as f64), object.clone()],
        )?;
        match mode {
            Walk::Every if !outcome.truthy() => return Ok(JsValue::Boolean(false)),
            Walk::Some if outcome.truthy() => return Ok(JsValue::Boolean(true)),
            Walk::Map => species::define(vm, result.expect("map result"), index, outcome)?,
            Walk::Filter if outcome.truthy() => {
                species::define(vm, result.expect("filter result"), filtered, value)?;
                filtered += 1;
            }
            Walk::Find if outcome.truthy() => return Ok(value),
            Walk::FindIndex if outcome.truthy() => return Ok(JsValue::Number(index as f64)),
            _ => {}
        }
    }
    Ok(match mode {
        Walk::Every => JsValue::Boolean(true),
        Walk::Some => JsValue::Boolean(false),
        Walk::ForEach | Walk::Find => JsValue::Undefined,
        Walk::FindIndex => JsValue::Number(-1.0),
        Walk::Map | Walk::Filter => JsValue::Object(result.expect("result array")),
    })
}

/** Array.prototype.every. */
fn every(vm: &mut Vm, call: Call) -> Res<JsValue> {
    walk(vm, call, "Array.prototype.every", Walk::Every, false)
}
/** Array.prototype.some. */
fn some(vm: &mut Vm, call: Call) -> Res<JsValue> {
    walk(vm, call, "Array.prototype.some", Walk::Some, false)
}
/** Array.prototype.forEach. */
fn for_each(vm: &mut Vm, call: Call) -> Res<JsValue> {
    walk(vm, call, "Array.prototype.forEach", Walk::ForEach, false)
}
/** Array.prototype.map. */
fn map(vm: &mut Vm, call: Call) -> Res<JsValue> {
    walk(vm, call, "Array.prototype.map", Walk::Map, false)
}
/** Array.prototype.filter. */
fn filter(vm: &mut Vm, call: Call) -> Res<JsValue> {
    walk(vm, call, "Array.prototype.filter", Walk::Filter, false)
}
/** Array.prototype.find. */
fn find(vm: &mut Vm, call: Call) -> Res<JsValue> {
    walk(vm, call, "Array.prototype.find", Walk::Find, false)
}
/** Array.prototype.findIndex. */
fn find_index(vm: &mut Vm, call: Call) -> Res<JsValue> {
    walk(
        vm,
        call,
        "Array.prototype.findIndex",
        Walk::FindIndex,
        false,
    )
}
/** Array.prototype.findLast. */
fn find_last(vm: &mut Vm, call: Call) -> Res<JsValue> {
    walk(vm, call, "Array.prototype.findLast", Walk::Find, true)
}
/** Array.prototype.findLastIndex. */
fn find_last_index(vm: &mut Vm, call: Call) -> Res<JsValue> {
    walk(
        vm,
        call,
        "Array.prototype.findLastIndex",
        Walk::FindIndex,
        true,
    )
}

/** Array.prototype.fill. */
fn fill(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.fill")?;
    let start = vm.relative_index(&call.arg(1), length, 0)?;
    let end = vm.relative_index(&call.arg(2), length, length)?;
    for index in start..end {
        set_index(vm, &object, index, call.arg(0))?;
    }
    Ok(object)
}

/** Array.prototype.flat preserves species construction and source read order. */
fn flat(vm: &mut Vm, call: Call) -> Res<JsValue> {
    flatten::flat(vm, call)
}

/** Array.prototype.flatMap flattens each mapped item before visiting the next input. */
fn flat_map(vm: &mut Vm, call: Call) -> Res<JsValue> {
    flatten::flat_map(vm, call)
}

/** Array.prototype.includes (SameValueZero, holes read as undefined). */
fn includes(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.includes")?;
    if length == 0 {
        return Ok(JsValue::Boolean(false));
    }
    let start = vm.relative_index(&call.arg(1), length, 0)?;
    let target = call.arg(0);
    for index in start..length {
        if get_index(vm, &object, index)?.same_value_zero(&target) {
            return Ok(JsValue::Boolean(true));
        }
    }
    Ok(JsValue::Boolean(false))
}

/** Array.prototype.indexOf (strict equality, skipping holes). */
fn index_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.indexOf")?;
    if length == 0 {
        return Ok(JsValue::Number(-1.0));
    }
    let start = vm.relative_index(&call.arg(1), length, 0)?;
    let target = call.arg(0);
    for index in start..length {
        if has_index(vm, &object, index)? && get_index(vm, &object, index)?.strict_equal(&target) {
            return Ok(JsValue::Number(index as f64));
        }
    }
    Ok(JsValue::Number(-1.0))
}

/** Array.prototype.lastIndexOf. */
fn last_index_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.lastIndexOf")?;
    if length == 0 {
        return Ok(JsValue::Number(-1.0));
    }
    let start = if call.args.len() > 1 {
        let relative = vm.to_integer(&call.arg(1))?;
        if relative < 0.0 {
            length as f64 + relative
        } else {
            relative.min(length as f64 - 1.0)
        }
    } else {
        length as f64 - 1.0
    };
    let target = call.arg(0);
    let mut index = start;
    while index >= 0.0 {
        let position = index as usize;
        if has_index(vm, &object, position)?
            && get_index(vm, &object, position)?.strict_equal(&target)
        {
            return Ok(JsValue::Number(index));
        }
        index -= 1.0;
    }
    Ok(JsValue::Number(-1.0))
}

/** Join elements with a separator; nullish elements become empty. */
pub(in crate::v9) fn join_values(
    vm: &mut Vm,
    object: &JsValue,
    length: usize,
    separator: &JsString,
) -> Res<JsString> {
    if separator.is_empty()
        && let Some(text) = join_sparse::empty(vm, object, length)?
    {
        return Ok(text);
    }
    let mut units: Vec<u16> = Vec::new();
    for index in 0..length {
        if index > 0 {
            units.extend_from_slice(separator.as_units());
        }
        let value = get_index(vm, object, index)?;
        if !value.nullish() {
            let text = vm.to_string(&value)?;
            units.extend_from_slice(text.as_units());
        }
        if units.len() > crate::constants::JS_STRING_UNITS {
            break;
        }
        vm.charge(1)?;
    }
    Ok(JsString::from_units(units)?)
}

/** Array.prototype.join. */
fn join(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.join")?;
    let separator = match call.arg(0) {
        JsValue::Undefined => JsString::from_utf8(",")?,
        value => vm.to_string(&value)?,
    };
    if vm.page.joining.contains(&object) {
        return Ok(JsValue::String(JsString::empty()));
    }
    vm.page.joining.push(object.clone());
    let result = join_values(vm, &object, length, &separator);
    vm.page.joining.pop();
    Ok(JsValue::String(result?))
}

/** Locale conversion invokes each present element's locale method, with shared cycle cleanup. */
fn to_locale_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.toLocaleString")?;
    if vm.page.joining.contains(&object) {
        return Ok(JsValue::String(JsString::empty()));
    }
    vm.page.joining.push(object.clone());
    let result = locale_elements(vm, &object, length, &call);
    vm.page.joining.pop();
    result
}

/** Locale element calls preserve their receivers and run in increasing index order. */
pub(super) fn locale_elements(
    vm: &mut Vm,
    object: &JsValue,
    length: usize,
    call: &Call,
) -> Res<JsValue> {
    let mut units = Vec::new();
    for index in 0..length {
        if index > 0 {
            units.push(u16::from(b','));
        }
        let value = get_index(vm, object, index)?;
        if !value.nullish() {
            let method = vm.get(&value, &key("toLocaleString"))?;
            if !vm.is_callable(&method) {
                let message = match method {
                    JsValue::Number(number) => format!(
                        "number {} is not a function",
                        crate::v9::value::number_to_string(number)
                    ),
                    _ => "toLocaleString is not a function".into(),
                };
                return Err(vm.type_error(&message));
            }
            let result = vm.call(method, value, vec![call.arg(0), call.arg(1)])?;
            units.extend_from_slice(vm.to_string(&result)?.as_units());
        }
        if units.len() > crate::constants::JS_STRING_UNITS {
            return Err(vm.range_error("Invalid string length"));
        }
        vm.charge(1)?;
    }
    Ok(JsValue::String(JsString::from_units(units)?))
}

/** Array.prototype.toString. */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_object(vm, &call, "Array.prototype.toString")?;
    let method = vm.get(&JsValue::Object(id), &key("join"))?;
    if vm.is_callable(&method) {
        return vm.call(method, JsValue::Object(id), Vec::new());
    }
    let tag = super::object::builtin_tag(vm, &JsValue::Object(id))?;
    text(vm, &format!("[object {tag}]"))
}

/** Array.prototype.pop. */
fn pop(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.pop")?;
    if length == 0 {
        set_length(vm, &object, 0)?;
        return Ok(JsValue::Undefined);
    }
    let value = get_index(vm, &object, length - 1)?;
    delete_index(vm, &object, length - 1)?;
    set_length(vm, &object, length - 1)?;
    Ok(value)
}

/** Array.prototype.push. */
fn push(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.push")?;
    if call.args.len() > crate::constants::VM_MAX_SAFE_LENGTH as usize - length {
        return Err(vm.type_error("Array.prototype.push exceeds the maximum safe length"));
    }
    for (offset, value) in call.args.iter().enumerate() {
        set_index(vm, &object, length + offset, value.clone())?;
    }
    set_length(vm, &object, length + call.args.len())?;
    Ok(JsValue::Number((length + call.args.len()) as f64))
}

/** Shared reduce loop. */
fn reduce_with(vm: &mut Vm, call: Call, reverse: bool) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.reduce")?;
    let function = callback(vm, &call, "Array.prototype.reduce")?;
    // Lazy index order: an array-like may claim length 2^53 - 1, which must never be allocated.
    let mut steps: Box<dyn Iterator<Item = usize>> = if reverse {
        Box::new((0..length).rev())
    } else {
        Box::new(0..length)
    };
    let mut accumulator = if call.args.len() > 1 {
        call.arg(1)
    } else {
        loop {
            match steps.next() {
                Some(index) if has_index(vm, &object, index)? => {
                    break get_index(vm, &object, index)?;
                }
                Some(_) => {}
                None => return Err(vm.type_error("Reduce of empty array with no initial value")),
            }
        }
    };
    for index in steps {
        vm.charge(1)?;
        if !has_index(vm, &object, index)? {
            continue;
        }
        let value = get_index(vm, &object, index)?;
        vm.pin(accumulator.clone());
        accumulator = vm.call(
            function.clone(),
            JsValue::Undefined,
            vec![
                accumulator,
                value,
                JsValue::Number(index as f64),
                object.clone(),
            ],
        )?;
    }
    Ok(accumulator)
}

/** Array.prototype.reduce. */
fn reduce(vm: &mut Vm, call: Call) -> Res<JsValue> {
    reduce_with(vm, call, false)
}
/** Array.prototype.reduceRight. */
fn reduce_right(vm: &mut Vm, call: Call) -> Res<JsValue> {
    reduce_with(vm, call, true)
}

/** The elements of an array-like as a vector with holes. */
fn snapshot(vm: &mut Vm, object: &JsValue, length: usize) -> Res<Vec<JsValue>> {
    let mut out = Vec::new();
    for index in 0..length {
        vm.charge(1)?;
        let value = if has_index(vm, object, index)? {
            get_index(vm, object, index)?
        } else {
            JsValue::Hole
        };
        check_length(vm, out.len() + 1)?;
        vm.pin(value.clone());
        out.push(value);
    }
    Ok(out)
}

/** Write a vector back into an array-like, deleting holes. */
fn write_back(vm: &mut Vm, object: &JsValue, values: Vec<JsValue>, _old_length: usize) -> Res<()> {
    for (index, value) in values.into_iter().enumerate() {
        if value == JsValue::Hole {
            delete_index(vm, object, index)?;
        } else {
            set_index(vm, object, index, value)?;
        }
    }
    Ok(())
}

/** Array.prototype.reverse. */
fn reverse(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.reverse")?;
    reverse_generic(vm, object, length)
}

/** Array.prototype.reverse's spec loop (23.1.3.26) for anything but a plain dense array: each
pair is read then written or deleted, and an odd length's middle element is never touched. */
fn reverse_generic(vm: &mut Vm, object: JsValue, length: usize) -> Res<JsValue> {
    for lower in 0..length / 2 {
        let upper = length - 1 - lower;
        let low = has_index(vm, &object, lower)?;
        let low_value = if low {
            Some(get_index(vm, &object, lower)?)
        } else {
            None
        };
        let high = has_index(vm, &object, upper)?;
        let high_value = if high {
            Some(get_index(vm, &object, upper)?)
        } else {
            None
        };
        match high_value {
            Some(value) => set_index(vm, &object, lower, value)?,
            None if low => delete_index(vm, &object, lower)?,
            None => continue,
        }
        match low_value {
            Some(value) => set_index(vm, &object, upper, value)?,
            None => delete_index(vm, &object, upper)?,
        }
    }
    Ok(object)
}

/** Array.prototype.shift. */
fn shift(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.shift")?;
    if length == 0 {
        set_length(vm, &object, 0)?;
        return Ok(JsValue::Undefined);
    }
    let first = get_index(vm, &object, 0)?;
    for index in 1..length {
        move_index(vm, &object, index, index - 1)?;
    }
    delete_index(vm, &object, length - 1)?;
    set_length(vm, &object, length - 1)?;
    Ok(first)
}

/** Array.prototype.unshift. */
fn unshift(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.unshift")?;
    let count = call.args.len();
    let new_length = safe_growth(vm, length, count)?;
    if count > 0 {
        for index in (0..length).rev() {
            move_index(vm, &object, index, index + count)?;
        }
        for (index, value) in call.args.iter().enumerate() {
            set_index(vm, &object, index, value.clone())?;
        }
    }
    set_length(vm, &object, new_length)?;
    Ok(JsValue::Number(new_length as f64))
}

/** Array.prototype.slice. */
fn slice(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.slice")?;
    let start = vm.relative_index(&call.arg(0), length, 0)?;
    let end = vm.relative_index(&call.arg(1), length, length)?;
    let result = species::create(vm, &object, end.saturating_sub(start))?;
    if !concat::slice_range(vm, result, &object, start, end)? {
        for index in start..end.max(start) {
            vm.charge(1)?;
            if has_index(vm, &object, index)? {
                let value = get_index(vm, &object, index)?;
                species::define(vm, result, index - start, value)?;
            }
        }
    }
    set_length(vm, &JsValue::Object(result), end.saturating_sub(start))?;
    Ok(JsValue::Object(result))
}

/** The default sort order: undefined last, then by string value. */
fn compare(vm: &mut Vm, comparator: &JsValue, a: &JsValue, b: &JsValue) -> Res<std::cmp::Ordering> {
    use std::cmp::Ordering;
    match (a, b) {
        (JsValue::Undefined, JsValue::Undefined) => return Ok(Ordering::Equal),
        (JsValue::Undefined, _) => return Ok(Ordering::Greater),
        (_, JsValue::Undefined) => return Ok(Ordering::Less),
        _ => {}
    }
    if !comparator.nullish() {
        let result = vm.call(
            comparator.clone(),
            JsValue::Undefined,
            vec![a.clone(), b.clone()],
        )?;
        let number = vm.to_number(&result)?;
        return Ok(if number < 0.0 {
            Ordering::Less
        } else if number > 0.0 {
            Ordering::Greater
        } else {
            Ordering::Equal
        });
    }
    let a = vm.to_string(a)?;
    let b = vm.to_string(b)?;
    Ok(a.as_units().cmp(b.as_units()))
}

/** A stable merge sort whose comparisons may run script code. */
fn merge_sort(vm: &mut Vm, comparator: &JsValue, values: Vec<JsValue>) -> Res<Vec<JsValue>> {
    if values.len() <= 1 {
        return Ok(values);
    }
    let mut runs: Vec<Vec<JsValue>> = values.into_iter().map(|value| vec![value]).collect();
    while runs.len() > 1 {
        let mut merged = Vec::with_capacity(runs.len().div_ceil(2));
        let mut iter = runs.into_iter();
        while let Some(left) = iter.next() {
            let Some(right) = iter.next() else {
                merged.push(left);
                break;
            };
            let mut out = Vec::with_capacity(left.len() + right.len());
            let (mut i, mut j) = (0, 0);
            while i < left.len() && j < right.len() {
                vm.charge(1)?;
                if compare(vm, comparator, &right[j], &left[i])? == std::cmp::Ordering::Less {
                    out.push(right[j].clone());
                    j += 1;
                } else {
                    out.push(left[i].clone());
                    i += 1;
                }
            }
            out.extend_from_slice(&left[i..]);
            out.extend_from_slice(&right[j..]);
            merged.push(out);
        }
        runs = merged;
    }
    Ok(runs.pop().unwrap_or_default())
}

/** Sort a snapshot: holes last, values pinned while the comparator runs. */
fn sorted(vm: &mut Vm, comparator: &JsValue, values: Vec<JsValue>) -> Res<Vec<JsValue>> {
    if !comparator.nullish() && !vm.is_callable(comparator) {
        return Err(vm.type_error("The comparison function must be either a function or undefined"));
    }
    let holes = values
        .iter()
        .filter(|value| **value == JsValue::Hole)
        .count();
    let present: Vec<JsValue> = values
        .into_iter()
        .filter(|value| *value != JsValue::Hole)
        .collect();
    for value in &present {
        vm.pin(value.clone());
    }
    let mut out = merge_sort(vm, comparator, present)?;
    out.extend(std::iter::repeat_n(JsValue::Hole, holes));
    Ok(out)
}

/** Invalid comparator diagnostics inspect primitive values without invoking an author's conversion. */
pub(super) fn sort_comparator(vm: &mut Vm, comparator: &JsValue) -> Res<()> {
    if *comparator == JsValue::Undefined || vm.is_callable(comparator) {
        return Ok(());
    }
    let value = match comparator {
        JsValue::Object(_) => "object".into(),
        JsValue::Symbol(_) => "Symbol()".into(),
        _ => vm.to_string(comparator)?.lossy(),
    };
    Err(vm.type_error(&format!(
        "The comparison function must be either a function or undefined: {value}"
    )))
}

/** Array.prototype.sort. */
fn sort(vm: &mut Vm, call: Call) -> Res<JsValue> {
    sort_sparse::run(vm, call)
}

/** Array.prototype.toSorted. */
fn to_sorted(vm: &mut Vm, call: Call) -> Res<JsValue> {
    copy::to_sorted(vm, call)
}

/** Splice arguments: start, delete count, inserted items. */
fn splice_parts(vm: &mut Vm, call: &Call, length: usize) -> Res<(usize, usize, Vec<JsValue>)> {
    let start = vm.relative_index(&call.arg(0), length, 0)?;
    let count = match call.args.len() {
        0 => 0,
        1 => length - start,
        _ => (vm.to_integer(&call.arg(1))?.max(0.0) as usize).min(length - start),
    };
    Ok((start, count, call.args.iter().skip(2).cloned().collect()))
}

/** Array.prototype.splice. */
fn splice(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.splice")?;
    let (start, count, items) = splice_parts(vm, &call, length)?;
    let new_length = safe_growth(vm, length - count, items.len())?;
    let removed = species::create(vm, &object, count)?;
    for index in 0..count {
        if has_index(vm, &object, start + index)? {
            let value = get_index(vm, &object, start + index)?;
            species::define(vm, removed, index, value)?;
        }
    }
    set_length(vm, &JsValue::Object(removed), count)?;
    splice_tail(vm, &object, length, start, count, items.len())?;
    for (offset, value) in items.into_iter().enumerate() {
        set_index(vm, &object, start + offset, value)?;
    }
    set_length(vm, &object, new_length)?;
    Ok(JsValue::Object(removed))
}

/** Moving an absent source deletes its destination instead of creating an undefined property. */
fn move_index(vm: &mut Vm, object: &JsValue, source: usize, target: usize) -> Res<()> {
    vm.charge(1)?;
    if has_index(vm, object, source)? {
        let value = get_index(vm, object, source)?;
        vm.pin(value.clone());
        set_index(vm, object, target, value)
    } else {
        delete_index(vm, object, target)
    }
}

/** Generic array lengths are safe integers even when they exceed actual array storage limits. */
fn safe_growth(vm: &mut Vm, length: usize, count: usize) -> Res<usize> {
    let max = crate::constants::VM_MAX_SAFE_LENGTH as usize;
    if count > max.saturating_sub(length) {
        return Err(vm.type_error("Array operation exceeds the maximum safe length"));
    }
    Ok(length + count)
}

/** Splice shifts only the affected suffix, in the direction that preserves overlapping reads. */
fn splice_tail(
    vm: &mut Vm,
    object: &JsValue,
    length: usize,
    start: usize,
    removed: usize,
    inserted: usize,
) -> Res<()> {
    if inserted < removed {
        for index in start..length - removed {
            move_index(vm, object, index + removed, index + inserted)?;
        }
        for index in (length - removed + inserted..length).rev() {
            delete_index(vm, object, index)?;
        }
    } else if inserted > removed {
        for index in (start..length - removed).rev() {
            move_index(vm, object, index + removed, index + inserted)?;
        }
    }
    Ok(())
}

/** Array.prototype.toSpliced. */
fn to_spliced(vm: &mut Vm, call: Call) -> Res<JsValue> {
    copy::to_spliced(vm, call)
}

/** Array.prototype.toReversed. */
fn to_reversed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    copy::to_reversed(vm, call)
}

/** Array.prototype.with. */
fn with(vm: &mut Vm, call: Call) -> Res<JsValue> {
    copy::with(vm, call)
}

/** The opt-in shell can disable Array subclass selection through its actual builtin-subclassing flag. */
fn builtin_subclassing(vm: &Vm) -> bool {
    #[cfg(feature = "conformance")]
    {
        vm.page.environment.conformance_builtin_subclassing
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        true
    }
}
