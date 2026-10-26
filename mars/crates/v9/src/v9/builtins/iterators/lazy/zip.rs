/** Joint iteration opens independent records once and closes live inputs in reverse acquisition order. */
use super::*;
use crate::v9::vm::heap::{Prop, Slot};
use state::State;
mod acquire;
mod advance;
pub(super) use advance::next;

/** Options are numeric private state, rather than author-visible coercible words. */
const SHORTEST: f64 = 0.0;
/** Longest mode retains a captured padding value for every input. */
const LONGEST: f64 = 1.0;
/** Strict mode requires all input done results to agree on the same step. */
const STRICT: f64 = 2.0;

/** Register both static forms with the specified arity. */
pub(super) fn install(builder: &mut Builder, constructor: ObjectId) {
    builder.method(constructor, "zip", 1, |vm, call| create(vm, call, false));
    builder.method(constructor, "zipKeyed", 1, |vm, call| {
        create(vm, call, true)
    });
}

/** Options and padding getters precede opening iterables; construction errors close acquired records. */
fn create(vm: &mut Vm, call: Call, keyed: bool) -> Res<JsValue> {
    if !matches!(call.arg(0), JsValue::Object(_)) {
        return Err(vm.type_error("Iterator.zip called on non-object"));
    }
    let (mode, padding) = options(vm, call.arg(1))?;
    let mut records = Vec::new();
    let mut keys = Vec::new();
    let acquired = if keyed {
        acquire::keyed(vm, call.arg(0), &mut records, &mut keys)
    } else {
        acquire::positional(vm, call.arg(0), &mut records)
    };
    if let Err(error) = acquired {
        return Err(close_records_error(vm, &mut records, error));
    }
    let pads = match acquire::padding(vm, padding, &keys, records.len(), keyed, mode == LONGEST) {
        Ok(pads) => pads,
        Err(error) => return Err(close_records_error(vm, &mut records, error)),
    };
    let record = vm.new_array(records)?;
    let padding = vm.new_array(pads)?;
    let keys = vm.new_array(keys)?;
    let payload = vm.new_object()?;
    put(vm, payload, "padding", JsValue::Object(padding));
    put(vm, payload, "keys", JsValue::Object(keys));
    let prototype = vm.intrinsic_id(*PROTOTYPE.get().expect("iterator helper installed"));
    State::create(
        vm,
        record,
        if keyed { Mode::ZipKeyed } else { Mode::Zip },
        JsValue::Object(payload),
        mode,
        prototype,
    )
}

/** Mode must already be one of the allowed strings; options and padding never undergo primitive boxing. */
fn options(vm: &mut Vm, options: JsValue) -> Res<(f64, JsValue)> {
    if matches!(options, JsValue::Undefined) {
        return Ok((SHORTEST, JsValue::Undefined));
    }
    if !matches!(options, JsValue::Object(_)) {
        return Err(vm.type_error("Iterator.zip options must be an object"));
    }
    let mode = match vm.get(&options, &Key::from("mode"))? {
        JsValue::Undefined => SHORTEST,
        JsValue::String(text) => match text.lossy().as_str() {
            "shortest" => SHORTEST,
            "longest" => LONGEST,
            "strict" => STRICT,
            _ => return Err(vm.type_error("Invalid Iterator.zip mode")),
        },
        _ => return Err(vm.type_error("Invalid Iterator.zip mode")),
    };
    let padding = if mode == LONGEST {
        vm.get(&options, &Key::from("padding"))?
    } else {
        JsValue::Undefined
    };
    if !matches!(padding, JsValue::Undefined | JsValue::Object(_)) {
        return Err(vm.type_error("Iterator.zip padding must be an object"));
    }
    Ok((mode, padding))
}

/** Returned helpers retain native arrays and immutable captured padding behind a private payload. */
fn get(vm: &Vm, id: ObjectId, name: &str) -> Res<JsValue> {
    match vm
        .heap
        .get(id)
        .and_then(|object| object.props.get(&Key::private(name)))
        .map(|prop| &prop.slot)
    {
        Some(Slot::Data(value)) => Ok(value.clone()),
        _ => Err(crate::protocol::EngineError::invalid("Invalid joint iterator state.").into()),
    }
}
/** Private slot writes never consult JavaScript setters. */
fn put(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) {
    vm.heap.put(id, Key::private(name), Prop::data(value));
}
/** Native array reads reject forged non-array saved state before selecting executable records. */
fn values(vm: &Vm, id: ObjectId) -> Res<Vec<JsValue>> {
    match vm.heap.get(id).map(|object| &object.class) {
        Some(Class::Array(array)) => Ok(array.iter().cloned().collect()),
        _ => Err(crate::protocol::EngineError::invalid("Invalid joint iterator array.").into()),
    }
}
/** Completion removes strong record references before invoking author return methods. */
fn clear(vm: &mut Vm, id: ObjectId, index: usize) {
    if let Some(Class::Array(array)) = vm.heap.get_mut(id).map(|object| &mut object.class) {
        array.set(index, JsValue::Undefined);
    }
}

/** Return closes every live input, retaining the first cleanup exception in reverse order. */
pub(super) fn close(vm: &mut Vm, state: &State) -> Res<()> {
    let mut records = values(vm, state.record)?;
    for index in 0..records.len() {
        clear(vm, state.record, index);
    }
    close_records(vm, &mut records)
}
/** Acquired records remain rooted by native pins during exception cleanup. */
fn close_records(vm: &mut Vm, records: &mut [JsValue]) -> Res<()> {
    let mut first = None;
    for record in records.iter_mut().rev() {
        if let Some(id) = record.object() {
            *record = JsValue::Undefined;
            if let Err(error) = vm.iter_close(id) {
                if matches!(error, Thrown::Fatal(_)) {
                    return Err(error);
                }
                if first.is_none() {
                    first = Some(error);
                }
            }
        }
    }
    match first {
        Some(error) => Err(error),
        None => Ok(()),
    }
}
/** An original script failure wins over script cleanup failures, while native termination still escapes. */
fn close_records_error(vm: &mut Vm, records: &mut [JsValue], error: Thrown) -> Thrown {
    match close_records(vm, records) {
        Err(Thrown::Fatal(fatal)) => Thrown::Fatal(fatal),
        _ => error,
    }
}
