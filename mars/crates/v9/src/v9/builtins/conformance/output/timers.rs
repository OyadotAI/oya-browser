/** D8 timer labels belong to the isolate, survive transport and measure the actual host-supplied VM clock. */
use super::*;
use crate::constants::VM_CONSOLE_ENTRIES;
use crate::protocol::EngineError;
use crate::v9::vm::GLOBAL;
use crate::v9::vm::Thrown;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{Key, Object, Prop, Slot};

/** A malformed private record is rejected instead of interpreting authored objects as native timer state. */
fn invalid() -> Thrown {
    EngineError::invalid("Invalid d8 console timer state.").into()
}

/** The genuine isolate-global root keeps the timer table independent of public console/global replacements. */
fn table(vm: &mut Vm) -> Res<ObjectId> {
    if let Some(property) = vm
        .heap
        .get(GLOBAL)
        .and_then(|object| object.props.get(&Key::private("d8-console-timers")))
    {
        let Slot::Data(JsValue::Object(id)) = property.slot else {
            return Err(invalid());
        };
        let Some(object) = vm.heap.get(id) else {
            return Err(invalid());
        };
        if object.proto.is_some()
            || object.realm != 0
            || !matches!(object.class, Class::Plain)
            || object.props.len() > VM_CONSOLE_ENTRIES
        {
            return Err(invalid());
        }
        for (name, property) in object.props.iter() {
            if !matches!(name, Key::Str(_))
                || !matches!(property.slot, Slot::Data(JsValue::Number(start)) if start.is_finite() && start >= 0.0 && start <= vm.page.state.clock)
            {
                return Err(invalid());
            }
        }
        return Ok(id);
    }
    let mut object = Object::new(Class::Plain, None);
    object.realm = 0;
    let id = vm.alloc(object)?;
    vm.pin(JsValue::Object(id));
    vm.heap.put(
        GLOBAL,
        Key::private("d8-console-timers"),
        Prop::data(JsValue::Object(id)),
    );
    Ok(id)
}

/** V8's TryCatch consumes ordinary label-conversion failures; native fatal failures still abort the operation. */
fn label(vm: &mut Vm, call: &Call) -> Res<Option<JsString>> {
    if call.args.is_empty() {
        return Ok(Some(JsString::from_utf8("default")?));
    }
    match vm.to_string(&call.arg(0)) {
        Ok(label) => Ok(Some(label)),
        Err(Thrown::Value(_)) => Ok(None),
        Err(error) => Err(error),
    }
}

/** Successful end removes the label; duplicate and missing labels produce the same concrete host diagnostics as d8. */
pub(super) fn timer(vm: &mut Vm, call: &Call, method: &str) -> Res<JsValue> {
    let Some(label) = label(vm, call)? else {
        return Ok(JsValue::Undefined);
    };
    let now = vm.page.state.clock;
    if method == "timeStamp" {
        vm.page.console(
            "log",
            format!("console.timeStamp: {}, {now:.6}", label.lossy()),
        );
        return Ok(JsValue::Undefined);
    }
    let id = table(vm)?;
    let key = Key::Str(label.clone());
    let start = vm
        .heap
        .get(id)
        .and_then(|object| object.props.get(&key))
        .and_then(|property| match property.slot {
            Slot::Data(JsValue::Number(start)) => Some(start),
            _ => None,
        });
    if method == "time" {
        return start_timer(vm, id, key, &label, start.is_some(), now);
    }
    if let Some(start) = start {
        vm.page.console(
            "log",
            format!("console.{method}: {}, {:.6}", label.lossy(), now - start),
        );
        if method == "timeEnd" {
            vm.heap.remove(id, &key);
        }
    } else {
        vm.page.console(
            "log",
            format!("console.{method}: Timer '{}' does not exist", label.lossy()),
        );
    }
    Ok(JsValue::Undefined)
}

/** Timer-table capacity uses the existing diagnostic entry limit instead of retaining unbounded user labels. */
fn start_timer(
    vm: &mut Vm,
    id: ObjectId,
    key: Key,
    label: &JsString,
    exists: bool,
    now: f64,
) -> Res<JsValue> {
    if exists {
        vm.page.console(
            "log",
            format!("console.time: Timer '{}' already exists", label.lossy()),
        );
    } else {
        if vm
            .heap
            .get(id)
            .is_none_or(|object| object.props.len() >= VM_CONSOLE_ENTRIES)
        {
            return Err(EngineError::limit("d8 console timer labels", VM_CONSOLE_ENTRIES).into());
        }
        vm.heap.put(id, key, Prop::data(JsValue::Number(now)));
    }
    Ok(JsValue::Undefined)
}
