/** Map, Set, WeakMap and WeakSet over insertion-ordered tables. */
use super::*;
use crate::v9::vm::data::{Class, IterKind, IterState, Table};
use crate::v9::vm::heap::{CONFIGURABLE, Object};
use crate::v9::vm::realm::{NativeFn, Static, realm, string, symbol};

mod construct;
mod set_algebra;
mod set_predicates;
mod set_record;
mod set_storage;

/** Which collection a constructor or method serves. */
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Map,
    Set,
    WeakMap,
    WeakSet,
}

/** The four collection constructors. */
pub(super) fn install(b: &mut Builder) {
    let object_prototype = b.ids.object_prototype;
    let map = b.object(Some(object_prototype));
    b.ids.map_prototype = map;
    let ctor = b.constructor(
        "Map",
        0,
        |vm, call| construct(vm, call, Kind::Map),
        map,
        None,
    );
    b.global("Map", ctor);
    super::typed::species_getter(b, ctor);
    let methods: [(&'static str, u32, NativeFn); 6] = [
        ("get", 1, |vm, call| map_get(vm, call, Kind::Map)),
        ("set", 2, |vm, call| map_set(vm, call, Kind::Map)),
        ("has", 1, |vm, call| has(vm, call, Kind::Map)),
        ("delete", 1, |vm, call| delete(vm, call, Kind::Map)),
        ("clear", 0, |vm, call| clear(vm, call, Kind::Map)),
        ("forEach", 1, |vm, call| for_each(vm, call, Kind::Map)),
    ];
    for (name, length, function) in methods {
        b.method(map, name, length, function);
    }
    b.builtin_accessor(
        map,
        "size",
        Some(|vm, call| size(vm, call, Kind::Map)),
        None,
    );
    b.method(map, "keys", 0, |vm, call| {
        iterate(vm, call, Kind::Map, IterKind::Keys)
    });
    b.method(map, "values", 0, |vm, call| {
        iterate(vm, call, Kind::Map, IterKind::Values)
    });
    let entries = b.method(map, "entries", 0, |vm, call| {
        iterate(vm, call, Kind::Map, IterKind::Entries)
    });
    b.put(
        map,
        Key::Sym(symbol::ITERATOR),
        Static::Value(JsValue::Object(entries)),
        crate::v9::vm::heap::HIDDEN_ATTRS,
    );
    b.put(
        map,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("Map")),
        CONFIGURABLE,
    );
    b.method(ctor, "groupBy", 2, group_by);

    let set = b.object(Some(object_prototype));
    b.ids.set_prototype = set;
    set_algebra::install(b, set);
    let ctor = b.constructor(
        "Set",
        0,
        |vm, call| construct(vm, call, Kind::Set),
        set,
        None,
    );
    b.global("Set", ctor);
    super::typed::species_getter(b, ctor);
    for (name, length, function) in [
        (
            "add",
            1,
            (|vm, call| set_add(vm, call, Kind::Set)) as NativeFn,
        ),
        ("has", 1, |vm, call| has(vm, call, Kind::Set)),
        ("delete", 1, |vm, call| delete(vm, call, Kind::Set)),
        ("clear", 0, |vm, call| clear(vm, call, Kind::Set)),
        ("forEach", 1, |vm, call| for_each(vm, call, Kind::Set)),
    ] {
        b.method(set, name, length, function);
    }
    b.builtin_accessor(
        set,
        "size",
        Some(|vm, call| size(vm, call, Kind::Set)),
        None,
    );
    b.method(set, "entries", 0, |vm, call| {
        iterate(vm, call, Kind::Set, IterKind::Entries)
    });
    let values = b.method(set, "values", 0, |vm, call| {
        iterate(vm, call, Kind::Set, IterKind::Values)
    });
    b.alias(set, "keys", values);
    b.put(
        set,
        Key::Sym(symbol::ITERATOR),
        Static::Value(JsValue::Object(values)),
        crate::v9::vm::heap::HIDDEN_ATTRS,
    );
    b.put(
        set,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("Set")),
        CONFIGURABLE,
    );

    let weak_map = b.object(Some(object_prototype));
    b.ids.weak_map_prototype = weak_map;
    let ctor = b.constructor(
        "WeakMap",
        0,
        |vm, call| construct(vm, call, Kind::WeakMap),
        weak_map,
        None,
    );
    b.global("WeakMap", ctor);
    for (name, length, function) in [
        (
            "get",
            1,
            (|vm, call| map_get(vm, call, Kind::WeakMap)) as NativeFn,
        ),
        ("set", 2, |vm, call| map_set(vm, call, Kind::WeakMap)),
        ("has", 1, |vm, call| has(vm, call, Kind::WeakMap)),
        ("delete", 1, |vm, call| delete(vm, call, Kind::WeakMap)),
    ] {
        b.method(weak_map, name, length, function);
    }
    b.put(
        weak_map,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("WeakMap")),
        CONFIGURABLE,
    );

    let weak_set = b.object(Some(object_prototype));
    b.ids.weak_set_prototype = weak_set;
    let ctor = b.constructor(
        "WeakSet",
        0,
        |vm, call| construct(vm, call, Kind::WeakSet),
        weak_set,
        None,
    );
    b.global("WeakSet", ctor);
    for (name, length, function) in [
        (
            "add",
            1,
            (|vm, call| set_add(vm, call, Kind::WeakSet)) as NativeFn,
        ),
        ("has", 1, |vm, call| has(vm, call, Kind::WeakSet)),
        ("delete", 1, |vm, call| delete(vm, call, Kind::WeakSet)),
    ] {
        b.method(weak_set, name, length, function);
    }
    b.put(
        weak_set,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("WeakSet")),
        CONFIGURABLE,
    );
}

/** `new Map(iterable)` and friends. */
fn construct(vm: &mut Vm, call: Call, kind: Kind) -> Res<JsValue> {
    let Some(target) = &call.new_target else {
        return Err(vm.type_error("Constructor requires 'new'"));
    };
    let ids = &realm().ids;
    let (class, fallback) = match kind {
        Kind::Map => (Class::Map(Box::default()), ids.map_prototype),
        Kind::Set => (Class::Set(Box::default()), ids.set_prototype),
        Kind::WeakMap => (Class::WeakMap(Box::default()), ids.weak_map_prototype),
        Kind::WeakSet => (Class::WeakSet(Box::default()), ids.weak_set_prototype),
    };
    let proto = vm.prototype_for(target, fallback)?;
    let id = vm.alloc(Object::new(class, Some(proto)))?;
    let source = call.arg(0);
    if source.nullish() {
        return Ok(JsValue::Object(id));
    }
    construct::populate(vm, id, kind, source)?;
    Ok(JsValue::Object(id))
}

/** The table a receiver holds, with its kind. */
fn table<'v>(vm: &'v mut Vm, call: &Call) -> Option<(ObjectId, &'v mut Table, Kind)> {
    let JsValue::Object(id) = call.this else {
        return None;
    };
    match vm.heap.get_mut(id).map(|o| &mut o.class)? {
        Class::Map(table) => Some((id, table, Kind::Map)),
        Class::Set(table) => Some((id, table, Kind::Set)),
        Class::WeakMap(table) => Some((id, table, Kind::WeakMap)),
        Class::WeakSet(table) => Some((id, table, Kind::WeakSet)),
        _ => None,
    }
}

/** The table of a receiver that must be exactly this kind: Map methods refuse a Set and the
weak collections refuse the strong ones, as each method checks its own internal slot. */
fn table_of<'v>(vm: &'v mut Vm, call: &Call, want: Kind) -> Res<(ObjectId, &'v mut Table)> {
    let ok = matches!(table(vm, call), Some((_, _, kind)) if kind == want);
    if !ok {
        return Err(incompatible(vm));
    }
    let (id, table, _) = table(vm, call).expect("checked above");
    Ok((id, table))
}

/** The receiver error. */
fn incompatible(vm: &mut Vm) -> crate::v9::vm::Thrown {
    vm.type_error("Method called on incompatible receiver")
}

/** Weak collections take only objects as keys. */
fn check_weak(vm: &mut Vm, kind: Kind, value: &JsValue) -> Res<()> {
    if matches!(kind, Kind::WeakMap | Kind::WeakSet) && !vm.can_hold_weakly(value) {
        return Err(vm.type_error(if matches!(kind, Kind::WeakSet) {
            "Invalid value used in weak set"
        } else {
            "Invalid value used as weak map key"
        }));
    }
    Ok(())
}

/** Map.prototype.get. */
fn map_get(vm: &mut Vm, call: Call, want: Kind) -> Res<JsValue> {
    let key_value = call.arg(0);
    let (_, table) = table_of(vm, &call, want)?;
    Ok(table.get(&key_value).cloned().unwrap_or(JsValue::Undefined))
}

/** Map.prototype.set. */
fn map_set(vm: &mut Vm, call: Call, kind: Kind) -> Res<JsValue> {
    let (key_value, value) = (call.arg(0), call.arg(1));
    table_of(vm, &call, kind)?;
    check_weak(vm, kind, &key_value)?;
    vm.heap.track_value(&key_value);
    vm.heap.track_value(&value);
    let units = (key_value.string_units() + value.string_units()) as isize;
    let added = table(vm, &call)
        .map(|(_, table, _)| table.set(key_value, value))
        .unwrap_or(false);
    vm.heap.adjust(isize::from(added), units);
    Ok(call.this)
}

/** Set.prototype.add. */
fn set_add(vm: &mut Vm, call: Call, kind: Kind) -> Res<JsValue> {
    let value = call.arg(0);
    table_of(vm, &call, kind)?;
    check_weak(vm, kind, &value)?;
    vm.heap.track_value(&value);
    let units = value.string_units() as isize;
    let added = table(vm, &call)
        .map(|(_, table, _)| table.set(value, JsValue::Undefined))
        .unwrap_or(false);
    if added {
        vm.heap.adjust(1, units);
    }
    Ok(call.this)
}

/** has. */
fn has(vm: &mut Vm, call: Call, want: Kind) -> Res<JsValue> {
    let key_value = call.arg(0);
    let (_, table) = table_of(vm, &call, want)?;
    Ok(JsValue::Boolean(table.has(&key_value)))
}

/** delete. */
fn delete(vm: &mut Vm, call: Call, want: Kind) -> Res<JsValue> {
    let key_value = call.arg(0);
    let (_, table) = table_of(vm, &call, want)?;
    let removed = table.delete(&key_value);
    if let Some((k, v)) = &removed {
        let units = (k.string_units() + v.string_units()) as isize;
        vm.heap.adjust(-1, -units);
    }
    Ok(JsValue::Boolean(removed.is_some()))
}

/** clear. */
fn clear(vm: &mut Vm, call: Call, want: Kind) -> Res<JsValue> {
    let (_, table) = table_of(vm, &call, want)?;
    let live = table.live as isize;
    table.clear();
    vm.heap.adjust(-live, 0);
    Ok(JsValue::Undefined)
}

/** size. */
fn size(vm: &mut Vm, call: Call, want: Kind) -> Res<JsValue> {
    let (_, table) = table_of(vm, &call, want)?;
    Ok(JsValue::Number(table.live as f64))
}

/** forEach over live entries, including ones added during the walk. */
fn for_each(vm: &mut Vm, call: Call, kind: Kind) -> Res<JsValue> {
    let (id, _) = table_of(vm, &call, kind)?;
    let function = call.arg(0);
    if !vm.is_callable(&function) {
        return Err(vm.type_error("forEach callback is not a function"));
    }
    let mut index = 0;
    while let Some(Class::Map(table) | Class::Set(table)) = vm.heap.get(id).map(|o| &o.class) {
        let Some(entry) = table.entries.get(index).cloned() else {
            break;
        };
        index += 1;
        let Some((key_value, value)) = entry else {
            continue;
        };
        let value = if kind == Kind::Set {
            key_value.clone()
        } else {
            value
        };
        vm.call(
            function.clone(),
            call.arg(1),
            vec![value, key_value, call.this.clone()],
        )?;
    }
    Ok(JsValue::Undefined)
}

/** keys, values and entries iterators. */
fn iterate(vm: &mut Vm, call: Call, want: Kind, kind: IterKind) -> Res<JsValue> {
    let (id, _) = table_of(vm, &call, want)?;
    vm.new_iterator(IterState::Table {
        table: id,
        index: 0,
        kind,
        done: false,
    })
    .map(JsValue::Object)
}

/** Map.groupBy. */
fn group_by(vm: &mut Vm, call: Call) -> Res<JsValue> {
    super::grouping::group(vm, call, false)
}
