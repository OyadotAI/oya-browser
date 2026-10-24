/** Map, Set, WeakMap and WeakSet over insertion-ordered tables. */
use super::*;
use crate::v9::vm::data::{Class, IterKind, IterState, Table};
use crate::v9::vm::heap::{CONFIGURABLE, Object};
use crate::v9::vm::realm::{NativeFn, Static, realm, string, symbol};

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
    let methods: [(&'static str, u32, NativeFn); 6] = [
        ("get", 1, map_get),
        ("set", 2, map_set),
        ("has", 1, has),
        ("delete", 1, delete),
        ("clear", 0, clear),
        ("forEach", 1, for_each),
    ];
    for (name, length, function) in methods {
        b.method(map, name, length, function);
    }
    b.builtin_accessor(map, "size", Some(size), None);
    b.method(map, "keys", 0, |vm, call| iterate(vm, call, IterKind::Keys));
    b.method(map, "values", 0, |vm, call| {
        iterate(vm, call, IterKind::Values)
    });
    let entries = b.method(map, "entries", 0, |vm, call| {
        iterate(vm, call, IterKind::Entries)
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
    let ctor = b.constructor(
        "Set",
        0,
        |vm, call| construct(vm, call, Kind::Set),
        set,
        None,
    );
    b.global("Set", ctor);
    for (name, length, function) in [
        ("add", 1, set_add as NativeFn),
        ("has", 1, has),
        ("delete", 1, delete),
        ("clear", 0, clear),
        ("forEach", 1, for_each),
    ] {
        b.method(set, name, length, function);
    }
    b.builtin_accessor(set, "size", Some(size), None);
    b.method(set, "entries", 0, |vm, call| {
        iterate(vm, call, IterKind::Entries)
    });
    let values = b.method(set, "values", 0, |vm, call| {
        iterate(vm, call, IterKind::Values)
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
        ("get", 1, map_get as NativeFn),
        ("set", 2, map_set),
        ("has", 1, has),
        ("delete", 1, delete),
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
        ("add", 1, set_add as NativeFn),
        ("has", 1, has),
        ("delete", 1, delete),
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
    let adder = vm.get(
        &JsValue::Object(id),
        &key(if matches!(kind, Kind::Map | Kind::WeakMap) {
            "set"
        } else {
            "add"
        }),
    )?;
    for item in vm.iterate_to_vec(&source)? {
        let args = if matches!(kind, Kind::Map | Kind::WeakMap) {
            if !matches!(item, JsValue::Object(_)) {
                return Err(vm.type_error("Iterator value is not an entry object"));
            }
            vec![vm.get(&item, &key("0"))?, vm.get(&item, &key("1"))?]
        } else {
            vec![item]
        };
        vm.call(adder.clone(), JsValue::Object(id), args)?;
    }
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

/** The receiver error. */
fn incompatible(vm: &mut Vm) -> crate::v9::vm::Thrown {
    vm.type_error("Method called on incompatible receiver")
}

/** Weak collections take only objects as keys. */
fn check_weak(vm: &mut Vm, kind: Kind, value: &JsValue) -> Res<()> {
    if matches!(kind, Kind::WeakMap | Kind::WeakSet)
        && !matches!(value, JsValue::Object(_) | JsValue::Symbol(_))
    {
        return Err(vm.type_error("Invalid value used as weak collection key"));
    }
    Ok(())
}

/** Map.prototype.get. */
fn map_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let key_value = call.arg(0);
    let Some((_, table, _)) = table(vm, &call) else {
        return Err(incompatible(vm));
    };
    Ok(table.get(&key_value).cloned().unwrap_or(JsValue::Undefined))
}

/** Map.prototype.set. */
fn map_set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (key_value, value) = (call.arg(0), call.arg(1));
    let kind = table(vm, &call).map(|(_, _, kind)| kind);
    let Some(kind) = kind.filter(|kind| matches!(kind, Kind::Map | Kind::WeakMap)) else {
        return Err(incompatible(vm));
    };
    check_weak(vm, kind, &key_value)?;
    let units = (key_value.string_units() + value.string_units()) as isize;
    let added = table(vm, &call)
        .map(|(_, table, _)| table.set(key_value, value))
        .unwrap_or(false);
    vm.heap.adjust(isize::from(added), units);
    Ok(call.this)
}

/** Set.prototype.add. */
fn set_add(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let value = call.arg(0);
    let kind = table(vm, &call).map(|(_, _, kind)| kind);
    let Some(kind) = kind.filter(|kind| matches!(kind, Kind::Set | Kind::WeakSet)) else {
        return Err(incompatible(vm));
    };
    check_weak(vm, kind, &value)?;
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
fn has(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let key_value = call.arg(0);
    let Some((_, table, _)) = table(vm, &call) else {
        return Err(incompatible(vm));
    };
    Ok(JsValue::Boolean(table.has(&key_value)))
}

/** delete. */
fn delete(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let key_value = call.arg(0);
    let Some((_, table, _)) = table(vm, &call) else {
        return Err(incompatible(vm));
    };
    let removed = table.delete(&key_value);
    if let Some((k, v)) = &removed {
        let units = (k.string_units() + v.string_units()) as isize;
        vm.heap.adjust(-1, -units);
    }
    Ok(JsValue::Boolean(removed.is_some()))
}

/** clear. */
fn clear(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some((_, table, _)) = table(vm, &call) else {
        return Err(incompatible(vm));
    };
    let live = table.live as isize;
    table.clear();
    vm.heap.adjust(-live, 0);
    Ok(JsValue::Undefined)
}

/** size. */
fn size(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some((_, table, _)) = table(vm, &call) else {
        return Err(incompatible(vm));
    };
    Ok(JsValue::Number(table.live as f64))
}

/** forEach over live entries, including ones added during the walk. */
fn for_each(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let function = call.arg(0);
    if !vm.is_callable(&function) {
        return Err(vm.type_error("forEach callback is not a function"));
    }
    let Some((id, _, kind)) = table(vm, &call) else {
        return Err(incompatible(vm));
    };
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
fn iterate(vm: &mut Vm, call: Call, kind: IterKind) -> Res<JsValue> {
    let Some((id, _, _)) = table(vm, &call) else {
        return Err(incompatible(vm));
    };
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
    let callback = call.arg(1);
    let map = vm.alloc(Object::new(
        Class::Map(Box::default()),
        Some(realm().ids.map_prototype),
    ))?;
    for (index, item) in vm.iterate_to_vec(&call.arg(0))?.into_iter().enumerate() {
        let group = vm.call(
            callback.clone(),
            JsValue::Undefined,
            vec![item.clone(), JsValue::Number(index as f64)],
        )?;
        let existing = match vm.heap.get(map).map(|o| &o.class) {
            Some(Class::Map(table)) => table.get(&group).cloned(),
            _ => None,
        };
        let list = match existing {
            Some(list) => list,
            None => {
                let list = JsValue::Object(vm.new_array(Vec::new())?);
                if let Some(Class::Map(table)) = vm.heap.get_mut(map).map(|o| &mut o.class) {
                    table.set(group, list.clone());
                }
                vm.heap.adjust(1, 0);
                list
            }
        };
        vm.array_append(&list, item)?;
    }
    Ok(JsValue::Object(map))
}
