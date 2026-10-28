/** Performance marks and measures retain real timed entries across the page state boundary. */
use crate::v9::value::{JsString, JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{Key, Prop, Slot};
use crate::v9::vm::{Call, Res, Vm};

/** Private entry storage is never exposed as an authored property. */
const ENTRIES: &str = "performance.entries";
/** Shell timelines advance on real monotonic time; normal page time remains deterministic. */
pub(super) fn now(vm: &Vm) -> f64 {
    #[cfg(feature = "conformance")]
    if vm.page.environment.conformance_d8 { return crate::v9::builtins::d8_time_origin_and_elapsed().1; }
    vm.page.state.clock
}
/** The shell time origin shares the same process anchor as its Date.now clock. */
pub(super) fn origin(vm: &Vm) -> f64 {
    #[cfg(feature = "conformance")]
    if vm.page.environment.conformance_d8 { return crate::v9::builtins::d8_time_origin_and_elapsed().0; }
    vm.page.state.epoch
}
/** The hidden brand prevents borrowing performance methods onto arbitrary objects. */
pub(super) const BRAND: &str = "performance.brand";
/** Entries carry a private brand so measure options cannot impersonate a mark. */
const ENTRY_BRAND: &str = "performance.entry";

/** Return the real entries owned by this performance receiver. */
fn entries(vm: &mut Vm, receiver: &JsValue) -> Res<(ObjectId, Vec<JsValue>)> {
    let id = receiver
        .object()
        .ok_or_else(|| vm.type_error("Invalid performance receiver"))?;
    vm.materialize(id)?;
    if vm
        .heap
        .get(id)
        .and_then(|object| object.props.get(&Key::private(BRAND)))
        .is_none()
    {
        return Err(vm.type_error("Invalid performance receiver"));
    }
    let list = vm
        .heap
        .get(id)
        .and_then(|o| o.props.get(&Key::private(ENTRIES)))
        .and_then(|p| match &p.slot {
            Slot::Data(value) => value.object(),
            _ => None,
        });
    let values = list
        .and_then(|id| vm.heap.get(id))
        .and_then(|o| match &o.class {
            Class::Array(values) => Some(values.iter().cloned().collect()),
            _ => None,
        })
        .unwrap_or_default();
    Ok((id, values))
}

/** Check the brand before any observable argument conversion. */
pub(super) fn check_receiver(vm: &mut Vm, receiver: &JsValue) -> Res<()> {
    entries(vm, receiver).map(|_| ())
}

/** Replace one rooted list after allocating the complete new list. */
fn save(vm: &mut Vm, id: ObjectId, values: Vec<JsValue>) -> Res<()> {
    let list = vm.new_array(values)?;
    vm.heap
        .put(id, Key::private(ENTRIES), Prop::data(JsValue::Object(list)));
    Ok(())
}

/** Allocate an entry with real name, type and virtual monotonic timeline values. */
fn entry(vm: &mut Vm, name: JsString, kind: &str, span: (f64, f64), detail: JsValue) -> Res<JsValue> {
    let (start, duration) = span;
    let id = vm.new_object()?;
    for (key, value) in [
        ("name", JsValue::String(name)),
        ("entryType", JsValue::string(kind)?),
        ("startTime", JsValue::Number(start)),
        ("duration", JsValue::Number(duration)),
        ("detail", detail),
    ] {
        vm.heap.put(
            id,
            Key::from(key),
            Prop {
                slot: Slot::Data(value),
                attrs: 0,
            },
        );
    }
    vm.heap.put(
        id,
        Key::private(ENTRY_BRAND),
        Prop::data(JsValue::Boolean(true)),
    );
    Ok(JsValue::Object(id))
}

/** Mark adds an actual entry at the current host timeline time. */
pub(super) fn mark(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, _) = entries(vm, &call.this)?;
    if call.args.is_empty() {
        return Err(vm.type_error("performance.mark requires a name"));
    }
    let name = vm.to_string(&call.arg(0))?;
    // User Timing 3 options: `startTime` (a timestamp, never negative) and `detail`.
    let options = call.arg(1);
    let start = match option(vm, &options, "startTime")? {
        Some(time) => vm.to_number(&time)?,
        None => now(vm),
    };
    if start < 0.0 {
        return Err(vm.type_error("performance.mark: startTime cannot be negative"));
    }
    let detail = option(vm, &options, "detail")?.unwrap_or(JsValue::Null);
    let (_, mut values) = entries(vm, &call.this)?;
    let value = entry(vm, name, "mark", (start, 0.0), detail)?;
    values.push(value.clone());
    save(vm, id, values)?;
    Ok(value)
}

/** A member of an options dictionary, None when absent or undefined (or no dictionary). */
fn option(vm: &mut Vm, options: &JsValue, name: &str) -> Res<Option<JsValue>> {
    if !matches!(options, JsValue::Object(_)) {
        return Ok(None);
    }
    let value = vm.get(options, &Key::from(name))?;
    Ok((value != JsValue::Undefined).then_some(value))
}

/** measure's second argument is an options dictionary when it is a plain object, not an entry. */
fn is_options(vm: &Vm, value: &JsValue) -> bool {
    value.object().is_some_and(|id| {
        vm.heap
            .get(id)
            .is_some_and(|object| object.props.get(&Key::private(ENTRY_BRAND)).is_none())
    })
}

/** `measure(name, {start, end, duration, detail})`: start and end are mark names or
timestamps; a missing one comes from the other and `duration`, else from 0 and now. */
fn measure_options(
    vm: &mut Vm,
    values: &[JsValue],
    options: &JsValue,
    now: f64,
) -> Res<((f64, f64), JsValue)> {
    let start = option(vm, options, "start")?;
    let end = option(vm, options, "end")?;
    let duration = match option(vm, options, "duration")? {
        Some(value) => Some(vm.to_number(&value)?),
        None => None,
    };
    let detail = option(vm, options, "detail")?.unwrap_or(JsValue::Null);
    let start = match start {
        Some(value) => Some(timestamp(vm, values, value)?),
        None => None,
    };
    let end = match end {
        Some(value) => Some(timestamp(vm, values, value)?),
        None => None,
    };
    let (from, to) = match (start, end, duration) {
        (Some(from), Some(to), _) => (from, to),
        (Some(from), None, Some(length)) => (from, from + length),
        (None, Some(to), Some(length)) => (to - length, to),
        (Some(from), None, None) => (from, now),
        (None, Some(to), None) => (0.0, to),
        (None, None, _) => (0.0, now),
    };
    Ok(((from, to - from), detail))
}

/** A measure endpoint: a number is a timestamp, anything else names a mark. */
fn timestamp(vm: &mut Vm, values: &[JsValue], value: JsValue) -> Res<f64> {
    match value.number_value() {
        Some(time) => Ok(time),
        None => position(vm, values, value, 0.0),
    }
}

/** Resolve an existing mark by its latest matching name or by the mark entry itself. */
fn position(vm: &mut Vm, values: &[JsValue], value: JsValue, fallback: f64) -> Res<f64> {
    if value == JsValue::Undefined {
        return Ok(fallback);
    }
    if let Some(id) = value.object() {
        if vm
            .heap
            .get(id)
            .and_then(|object| object.props.get(&Key::private(ENTRY_BRAND)))
            .is_none()
        {
            return Err(vm.unsupported_error("performance.measure options"));
        }
        let start = vm.get(&value, &Key::from("startTime"))?;
        if start == JsValue::Undefined {
            return Err(vm.unsupported_error("performance.measure options"));
        }
        return vm.to_number(&start);
    }
    let name = vm.to_string(&value)?;
    for item in values.iter().rev() {
        if vm.get(item, &Key::from("entryType"))? == JsValue::string("mark")?
            && vm.get(item, &Key::from("name"))? == JsValue::String(name.clone())
        {
            let value = vm.get(item, &Key::from("startTime"))?;
            return vm.to_number(&value);
        }
    }
    Err(vm.throw(
        crate::v9::vm::error::ErrorKind::Error,
        "Unknown performance mark",
    ))
}

/** Measures use actual marks and record the elapsed timeline interval. */
pub(super) fn measure(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (id, _) = entries(vm, &call.this)?;
    if call.args.is_empty() {
        return Err(vm.type_error("performance.measure requires a name"));
    }
    let name = vm.to_string(&call.arg(0))?;
    let (_, mut values) = entries(vm, &call.this)?;
    let now = now(vm);
    if is_options(vm, &call.arg(1)) {
        let (span, detail) = measure_options(vm, &values, &call.arg(1), now)?;
        let value = entry(vm, name, "measure", span, detail)?;
        values.push(value.clone());
        save(vm, id, values)?;
        return Ok(value);
    }
    let start = position(vm, &values, call.arg(1), 0.0)?;
    let end = position(vm, &values, call.arg(2), now)?;
    let value = entry(vm, name, "measure", (start, end - start), JsValue::Null)?;
    values.push(value.clone());
    save(vm, id, values)?;
    Ok(value)
}

/** Retrieve entries by optional type and name without exposing their backing list. */
pub(super) fn get_entries(
    vm: &mut Vm,
    call: Call,
    kind: Option<&str>,
    name: Option<JsString>,
) -> Res<JsValue> {
    let (_, values) = entries(vm, &call.this)?;
    let mut selected = Vec::new();
    for value in values {
        if let Some(kind) = kind
            && vm.get(&value, &Key::from("entryType"))? != JsValue::string(kind)?
        {
            continue;
        }
        if let Some(name) = &name
            && vm.get(&value, &Key::from("name"))? != JsValue::String(name.clone())
        {
            continue;
        }
        let start = vm.get(&value, &Key::from("startTime"))?;
        selected.push((vm.to_number(&start)?, value));
    }
    selected.sort_by(|left, right| left.0.total_cmp(&right.0));
    Ok(JsValue::Object(vm.new_array(
        selected.into_iter().map(|(_, value)| value).collect(),
    )?))
}

/** Clearing removes only matching entry kinds and names. */
pub(super) fn clear(vm: &mut Vm, call: Call, kind: &str) -> Res<JsValue> {
    let (id, _) = entries(vm, &call.this)?;
    let name = if call.arg(0) == JsValue::Undefined {
        None
    } else {
        Some(vm.to_string(&call.arg(0))?)
    };
    let (_, values) = entries(vm, &call.this)?;
    let mut retained = Vec::new();
    for value in values {
        let same_kind = vm.get(&value, &Key::from("entryType"))? == JsValue::string(kind)?;
        let same_name = match &name {
            Some(name) => vm.get(&value, &Key::from("name"))? == JsValue::String(name.clone()),
            None => true,
        };
        if !same_kind || !same_name {
            retained.push(value);
        }
    }
    save(vm, id, retained)?;
    Ok(JsValue::Undefined)
}
