/** D8's memory API measures reachable V9 resident data asynchronously and resolves through real Promise semantics. */
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::memory::{BytesRange, MemoryEstimate};
use crate::v9::vm::property::Descriptor;
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};
use std::sync::OnceLock;

/** The native task identity is immutable; every pending request captures its own real promise and mode. */
static MEASURE_TASK: OnceLock<u32> = OnceLock::new();
/** Registration precedes platform.performance creation, which installs the public native method. */
pub(super) fn install(builder: &mut Builder) {
    let _ = MEASURE_TASK.set(builder.native("", 0, complete));
}
/** Only a literal boolean true requests detailed mode; option getters execute synchronously before allocation. */
pub(in crate::v9) fn measure(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let options = call.arg(0);
    let detailed = options.object().is_some()
        && vm.get(&options, &super::super::key("detailed"))? == JsValue::Boolean(true);
    let promise = vm.new_promise()?;
    vm.pin(JsValue::Object(promise));
    let task = vm.native_closure(
        *MEASURE_TASK.get().unwrap(),
        vec![JsValue::Object(promise), JsValue::Boolean(detailed)],
    )?;
    crate::v9::platform::schedule_task(vm, JsValue::Object(task), Vec::new())?;
    Ok(JsValue::Object(promise))
}
/** Collection and accounting happen in a later task; the callback and resolver are pinned native roots. */
fn complete(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let slots = vm.slots(call.callee);
    let Some(JsValue::Object(promise)) = slots.first().cloned() else {
        return Err(vm.type_error("Memory measurement task lost its resolver"));
    };
    let detailed = slots.get(1) == Some(&JsValue::Boolean(true));
    vm.collect_now()?;
    vm.charge(vm.heap.objects.len() + vm.heap.envs.len())?;
    let estimate = vm.memory_estimate();
    let result = result(vm, estimate, detailed)?;
    vm.resolve_promise(promise, JsValue::Object(result))?;
    Ok(JsValue::Undefined)
}
/** Native results use CreateDataProperty so inherited setters cannot intercept the measurement fields. */
fn put(vm: &mut Vm, object: ObjectId, name: &str, value: JsValue) -> Res<()> {
    if !vm.define(object, super::super::key(name), Descriptor::data(value))? {
        return Err(vm.type_error("Memory measurement result property was refused"));
    }
    Ok(())
}
/** The conservative estimate is the known resident lower bound; ranges additionally cover attribution/slack uncertainty. */
fn field(vm: &mut Vm, bytes: BytesRange) -> Res<JsValue> {
    let object = vm.new_object()?;
    vm.pin(JsValue::Object(object));
    let range = vm.new_array(vec![
        JsValue::Number(bytes.lower as f64),
        JsValue::Number(bytes.upper as f64),
    ])?;
    put(
        vm,
        object,
        "jsMemoryEstimate",
        JsValue::Number(bytes.lower as f64),
    )?;
    put(vm, object, "jsMemoryRange", JsValue::Object(range))?;
    Ok(JsValue::Object(object))
}
/** Detailed results name only contexts permitted by the requesting context's security token. */
fn result(vm: &mut Vm, estimate: MemoryEstimate, detailed: bool) -> Res<ObjectId> {
    let object = vm.new_object()?;
    vm.pin(JsValue::Object(object));
    let total = field(vm, estimate.total)?;
    put(vm, object, "total", total)?;
    if detailed {
        let current = field(vm, estimate.current)?;
        put(vm, object, "current", current)?;
        let mut others = Vec::new();
        for bytes in estimate.other {
            others.push(field(vm, bytes)?);
        }
        let others = vm.new_array(others)?;
        put(vm, object, "other", JsValue::Object(others))?;
    }
    Ok(object)
}
