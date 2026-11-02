/** The shell's getV8Statistics reports V9's measured heap bytes, defined as data so setters on the prototype never run. */
use super::*;
use crate::v9::value::ObjectId;
use crate::v9::vm::gc::PROMOTION_AGE;
use crate::v9::vm::heap::{Key, Object, Prop};
use crate::v9::vm::property::Descriptor;
use crate::v9::vm::realm::banks;
use std::mem::size_of;

/** The V8 counter names this host reports; V8's own misspelling of "committed" is kept for compatibility. */
const TOTAL_NAME: &str = "total_committed_bytes";
/** Live bytes of the young generation, the only generation V9 tracks by age. */
const NEW_SPACE_NAME: &str = "new_space_live_bytes";
/** V9 has no reserved-but-unused young space, so committed and live bytes are the same measurement. */
const NEW_SPACE_COMMITTED_NAME: &str = "new_space_commited_bytes";

/** The native identity is installed once and copied into each realm that the shell flag exposes. */
static NATIVE: OnceLock<ObjectId> = OnceLock::new();

/** Registration is independent of the flag; exposure decides whether scripts can name the function. */
pub(super) fn install(builder: &mut Builder) {
    let _ = NATIVE.set(builder.function("getV8Statistics", 0, statistics));
}

/** Only a realm created under the explicit shell flag receives getV8Statistics. */
pub(super) fn expose(vm: &mut Vm, global: ObjectId, bank: u32) {
    if !vm.page.environment.conformance_expose_statistics {
        return;
    }
    let id = *NATIVE.get().unwrap() + bank * banks::BANK_STRIDE;
    vm.materialize(id).expect("installed statistics intrinsic");
    vm.heap.put(
        global,
        Key::from("getV8Statistics"),
        Prop::data(JsValue::Object(id)),
    );
}

/** A young object's shell and its property storage are what V8's new space would hold for it. */
fn young_bytes(vm: &Vm) -> usize {
    vm.heap
        .objects
        .values()
        .filter(|object| object.age < PROMOTION_AGE)
        .map(object_bytes)
        .sum()
}

/** The shell's size plus the lower bound of its property storage, excluding shared allocations. */
fn object_bytes(object: &Object) -> usize {
    let (lower, _) = object.props.memory_bounds();
    size_of::<Object>().saturating_add(lower)
}

/** Each counter is defined as an own data property, as the native shell does with CreateDataProperty. */
fn put(vm: &mut Vm, object: ObjectId, name: &str, value: usize) -> Res<()> {
    let defined = vm.define(
        object,
        crate::v9::builtins::key(name),
        Descriptor::data(JsValue::Number(value as f64)),
    )?;
    if !defined {
        return Err(vm.type_error("Statistics property was refused"));
    }
    Ok(())
}

/** The result is a fresh plain object; every number comes from the same measurement pass. */
fn statistics(vm: &mut Vm, _call: Call) -> Res<JsValue> {
    let total = vm.memory_estimate().total.lower;
    let young = young_bytes(vm);
    let object = vm.new_object()?;
    vm.pin(JsValue::Object(object));
    put(vm, object, TOTAL_NAME, total)?;
    put(vm, object, NEW_SPACE_NAME, young)?;
    put(vm, object, NEW_SPACE_COMMITTED_NAME, young)?;
    Ok(JsValue::Object(object))
}
