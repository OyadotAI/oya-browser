/** Array: the constructor, statics and every ES2023 prototype method, generic over array-likes. */
use super::*;
use crate::protocol::EngineError;
use crate::v9::vm::data::{Class, IterKind, IterState};
use crate::v9::vm::realm::{NativeFn, symbol};

mod from_async;

/** Array and Array.prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.array_prototype = proto;
    let ctor = b.constructor("Array", 1, construct, proto, None);
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
        ("toLocaleString", 0, join_default),
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
    let elements = if let [JsValue::Number(length)] = call.args {
        if *length < 0.0 || length.fract() != 0.0 || *length > f64::from(u32::MAX) {
            return Err(vm.range_error("Invalid array length"));
        }
        if *length as usize > vm.limits.array_length {
            return Err(EngineError::limit("V9 array length", vm.limits.array_length).into());
        }
        vec![JsValue::Hole; *length as usize]
    } else {
        call.args.to_vec()
    };
    let id = vm.new_array(elements)?;
    if let Some(target) = &call.new_target {
        let proto = vm.prototype_for(target, crate::v9::vm::realm::realm().ids.array_prototype)?;
        vm.set_proto(id, Some(proto))?;
    }
    Ok(JsValue::Object(id))
}

/** Whether a value is an Array. */
pub(in crate::v9) fn is_array_value(vm: &Vm, value: &JsValue) -> bool {
    matches!(value, JsValue::Object(id) if matches!(vm.heap.get(*id).map(|o| &o.class), Some(Class::Array(_))))
}

/** Array.isArray. */
fn is_array(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(is_array_value(vm, &call.arg(0))))
}

/** Array.of. */
fn of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Object(vm.new_array(call.args.to_vec())?))
}

/** Array.from(iterable or array-like, mapFn, thisArg). */
fn from(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let source = call.arg(0);
    let mapper = call.arg(1);
    let this_arg = call.arg(2);
    if source.nullish() {
        return Err(vm.type_error("Array.from requires an array-like object"));
    }
    let iterable = !vm.get(&source, &Key::Sym(symbol::ITERATOR))?.nullish();
    let items = if iterable {
        vm.iterate_to_vec(&source)?
    } else {
        let length = vm.length_of(&source)?;
        check_length(vm, length)?;
        let mut items = Vec::with_capacity(length);
        for index in 0..length {
            items.push(get_index(vm, &source, index)?);
        }
        items
    };
    let result = vm.new_array(Vec::new())?;
    for (index, item) in items.into_iter().enumerate() {
        let value = if mapper.nullish() {
            item
        } else {
            vm.call(
                mapper.clone(),
                this_arg.clone(),
                vec![item, JsValue::Number(index as f64)],
            )?
        };
        vm.array_append(&JsValue::Object(result), value)?;
    }
    Ok(JsValue::Object(result))
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
    Ok(is_array_value(vm, value))
}

/** Array.prototype.concat. */
fn concat(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_object(vm, &call, "Array.prototype.concat")?;
    let result = vm.new_array(Vec::new())?;
    let mut items = vec![JsValue::Object(id)];
    items.extend(call.args.iter().cloned());
    let mut out = Vec::new();
    for item in items {
        if spreadable(vm, &item)? {
            let length = vm.length_of(&item)?;
            check_length(vm, out.len() + length)?;
            for index in 0..length {
                out.push(if has_index(vm, &item, index)? {
                    get_index(vm, &item, index)?
                } else {
                    JsValue::Hole
                });
            }
        } else {
            check_length(vm, out.len() + 1)?;
            out.push(item);
        }
    }
    replace_elements(vm, result, out);
    Ok(JsValue::Object(result))
}

/** Replace a fresh array's elements, keeping counts. */
fn replace_elements(vm: &mut Vm, id: ObjectId, elements: Vec<JsValue>) {
    let count = elements
        .iter()
        .filter(|value| **value != JsValue::Hole)
        .count() as isize;
    let units = elements.iter().map(JsValue::string_units).sum::<usize>() as isize;
    if let Some(object) = vm.heap.get_mut(id)
        && let Class::Array(current) = &mut object.class
    {
        *current = elements;
        vm.heap.adjust(count, units);
    }
}

/** Array.prototype.copyWithin. */
fn copy_within(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.copyWithin")?;
    let target = vm.relative_index(&call.arg(0), length, 0)?;
    let start = vm.relative_index(&call.arg(1), length, 0)?;
    let end = vm.relative_index(&call.arg(2), length, length)?;
    let count = end.saturating_sub(start).min(length - target);
    let values: Vec<(bool, JsValue)> = (start..start + count)
        .map(|index| {
            Ok((
                has_index(vm, &object, index)?,
                get_index(vm, &object, index)?,
            ))
        })
        .collect::<Res<_>>()?;
    for (offset, (present, value)) in values.into_iter().enumerate() {
        if present {
            set_index(vm, &object, target + offset, value)?;
        } else {
            delete_index(vm, &object, target + offset)?;
        }
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
        Walk::Map => {
            check_length(vm, length)?;
            Some(vm.new_array(vec![JsValue::Hole; length])?)
        }
        Walk::Filter => Some(vm.new_array(Vec::new())?),
        _ => None,
    };
    let skips_holes = !matches!(mode, Walk::Find | Walk::FindIndex);
    for step in 0..length {
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
            Walk::Map => vm.array_store(result.expect("map result"), index, outcome),
            Walk::Filter if outcome.truthy() => {
                vm.array_append(&JsValue::Object(result.expect("filter result")), value)?
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

/** Flatten into `out` down to `depth`. */
fn flatten(vm: &mut Vm, source: &JsValue, depth: f64, out: &mut Vec<JsValue>) -> Res<()> {
    let length = vm.length_of(source)?;
    for index in 0..length {
        if !has_index(vm, source, index)? {
            continue;
        }
        let value = get_index(vm, source, index)?;
        if depth >= 1.0 && is_array_value(vm, &value) {
            flatten(vm, &value, depth - 1.0, out)?;
        } else {
            check_length(vm, out.len() + 1)?;
            out.push(value);
        }
    }
    Ok(())
}

/** Array.prototype.flat. */
fn flat(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_object(vm, &call, "Array.prototype.flat")?;
    let depth = if call.arg(0) == JsValue::Undefined {
        1.0
    } else {
        vm.to_integer(&call.arg(0))?
    };
    let mut out = Vec::new();
    flatten(
        vm,
        &JsValue::Object(id),
        depth.min(MAX_FLAT_DEPTH),
        &mut out,
    )?;
    Ok(JsValue::Object(vm.new_array(out)?))
}

/** flat() recursion depth, bounded below the syntax depth. */
const MAX_FLAT_DEPTH: f64 = 64.0;

/** Array.prototype.flatMap. */
fn flat_map(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mapped = walk(
        vm,
        Call {
            this: call.this.clone(),
            args: call.args,
            new_target: None,
            callee: call.callee,
        },
        "Array.prototype.flatMap",
        Walk::Map,
        false,
    )?;
    vm.pin(mapped.clone());
    let mut out = Vec::new();
    flatten(vm, &mapped, 1.0, &mut out)?;
    Ok(JsValue::Object(vm.new_array(out)?))
}

/** Array.prototype.includes (SameValueZero, holes read as undefined). */
fn includes(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.includes")?;
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

/** toLocaleString joins with commas. */
fn join_default(vm: &mut Vm, call: Call) -> Res<JsValue> {
    join(
        vm,
        Call {
            this: call.this.clone(),
            args: &[],
            new_target: None,
            callee: call.callee,
        },
    )
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
    if is_array_value(vm, &object) {
        for value in call.args {
            vm.array_append(&object, value.clone())?;
        }
        return Ok(JsValue::Number((length + call.args.len()) as f64));
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
    check_length(vm, length)?;
    let mut out = Vec::with_capacity(length);
    for index in 0..length {
        out.push(if has_index(vm, object, index)? {
            get_index(vm, object, index)?
        } else {
            JsValue::Hole
        });
    }
    Ok(out)
}

/** Write a vector back into an array-like, deleting holes. */
fn write_back(vm: &mut Vm, object: &JsValue, values: Vec<JsValue>, old_length: usize) -> Res<()> {
    let length = values.len();
    if let JsValue::Object(id) = object
        && is_array_value(vm, object)
        && vm.heap.get(*id).is_some_and(|o| o.props.is_empty())
    {
        if let Some(Class::Array(elements)) = vm.heap.get(*id).map(|o| &o.class) {
            let count = elements
                .iter()
                .filter(|value| **value != JsValue::Hole)
                .count() as isize;
            let units = elements.iter().map(JsValue::string_units).sum::<usize>() as isize;
            vm.heap.adjust(-count, -units);
        }
        if let Some(Class::Array(elements)) = vm.heap.get_mut(*id).map(|o| &mut o.class) {
            *elements = Vec::new();
        }
        replace_elements(vm, *id, values);
        return Ok(());
    }
    for (index, value) in values.into_iter().enumerate() {
        if value == JsValue::Hole {
            delete_index(vm, object, index)?;
        } else {
            set_index(vm, object, index, value)?;
        }
    }
    for index in length..old_length {
        delete_index(vm, object, index)?;
    }
    set_length(vm, object, length)
}

/** Array.prototype.reverse. */
fn reverse(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.reverse")?;
    let dense = matches!(&object, JsValue::Object(id) if is_array_value(vm, &object)
        && vm.heap.get(*id).is_some_and(|o| o.props.is_empty()));
    if !dense {
        return reverse_generic(vm, object, length);
    }
    let mut values = snapshot(vm, &object, length)?;
    values.reverse();
    write_back(vm, &object, values, length)?;
    Ok(object)
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
    let mut values = snapshot(vm, &object, length)?;
    let first = values.remove(0);
    write_back(vm, &object, values, length)?;
    Ok(if first == JsValue::Hole {
        JsValue::Undefined
    } else {
        first
    })
}

/** Array.prototype.unshift. */
fn unshift(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.unshift")?;
    let mut values = snapshot(vm, &object, length)?;
    check_length(vm, length + call.args.len())?;
    values.splice(0..0, call.args.iter().cloned());
    let new_length = values.len();
    write_back(vm, &object, values, length)?;
    Ok(JsValue::Number(new_length as f64))
}

/** Array.prototype.slice. */
fn slice(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.slice")?;
    let start = vm.relative_index(&call.arg(0), length, 0)?;
    let end = vm.relative_index(&call.arg(1), length, length)?;
    let mut out = Vec::with_capacity(end.saturating_sub(start));
    for index in start..end.max(start) {
        out.push(if has_index(vm, &object, index)? {
            get_index(vm, &object, index)?
        } else {
            JsValue::Hole
        });
    }
    Ok(JsValue::Object(vm.new_array(out)?))
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

/** Array.prototype.sort. */
fn sort(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.sort")?;
    let values = snapshot(vm, &object, length)?;
    let values = sorted(vm, &call.arg(0), values)?;
    write_back(vm, &object, values, length)?;
    Ok(object)
}

/** Array.prototype.toSorted. */
fn to_sorted(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.toSorted")?;
    let values: Vec<JsValue> = snapshot(vm, &object, length)?
        .into_iter()
        .map(|value| {
            if value == JsValue::Hole {
                JsValue::Undefined
            } else {
                value
            }
        })
        .collect();
    let values = sorted(vm, &call.arg(0), values)?;
    Ok(JsValue::Object(vm.new_array(values)?))
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
    let mut values = snapshot(vm, &object, length)?;
    check_length(vm, length - count + items.len())?;
    let removed: Vec<JsValue> = values.splice(start..start + count, items).collect();
    write_back(vm, &object, values, length)?;
    Ok(JsValue::Object(vm.new_array(removed)?))
}

/** Array.prototype.toSpliced. */
fn to_spliced(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.toSpliced")?;
    let (start, count, items) = splice_parts(vm, &call, length)?;
    let mut values: Vec<JsValue> = snapshot(vm, &object, length)?
        .into_iter()
        .map(|value| {
            if value == JsValue::Hole {
                JsValue::Undefined
            } else {
                value
            }
        })
        .collect();
    check_length(vm, length - count + items.len())?;
    values.splice(start..start + count, items);
    Ok(JsValue::Object(vm.new_array(values)?))
}

/** Array.prototype.toReversed. */
fn to_reversed(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.toReversed")?;
    let mut values: Vec<JsValue> = snapshot(vm, &object, length)?
        .into_iter()
        .map(|value| {
            if value == JsValue::Hole {
                JsValue::Undefined
            } else {
                value
            }
        })
        .collect();
    values.reverse();
    Ok(JsValue::Object(vm.new_array(values)?))
}

/** Array.prototype.with. */
fn with(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (object, length) = receiver(vm, &call, "Array.prototype.with")?;
    let relative = vm.to_integer(&call.arg(0))?;
    let index = if relative < 0.0 {
        length as f64 + relative
    } else {
        relative
    };
    if index < 0.0 || index >= length as f64 {
        return Err(vm.range_error("Invalid index"));
    }
    let mut values: Vec<JsValue> = snapshot(vm, &object, length)?
        .into_iter()
        .map(|value| {
            if value == JsValue::Hole {
                JsValue::Undefined
            } else {
                value
            }
        })
        .collect();
    values[index as usize] = call.arg(1);
    Ok(JsValue::Object(vm.new_array(values)?))
}
