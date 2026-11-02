/** The shell's failure triggers abort the isolate like CHECK, and assert-only failures fire only where DCHECK does. */
use super::*;
use crate::protocol::EngineError;
use crate::v9::value::ObjectId;
use crate::v9::vm::heap::{Key, Prop};
use crate::v9::vm::realm::banks;

/** The trigger name exposed by the original `--expose-trigger-failure` extension. */
const CHECK_NAME: &str = "triggerCheckFalse";
/** The assert-only trigger name exposed next to the check trigger. */
const ASSERT_NAME: &str = "triggerAssertFalse";
/** Error code reported when a check fails, distinct from script exceptions and host exits. */
const CHECK_CODE: &str = "conformance_check_failed";

/** Native identities are installed once; each fresh realm exposes its own copy under the requested flag. */
static CHECK: OnceLock<ObjectId> = OnceLock::new();
/** The assert trigger shares the native check's identity scheme, so banks never alias. */
static ASSERT: OnceLock<ObjectId> = OnceLock::new();

/** Registers both callbacks without making either visible to scripts until a realm is exposed. */
pub(super) fn install(builder: &mut Builder) {
    let _ = CHECK.set(builder.function(CHECK_NAME, 0, check_false));
    let _ = ASSERT.set(builder.function(ASSERT_NAME, 0, assert_false));
}

/** Only a realm created under the explicit shell flag receives the globals; other realms keep them absent. */
pub(super) fn expose(vm: &mut Vm, global: ObjectId, bank: u32) {
    if !vm.page.environment.conformance_expose_trigger_failure {
        return;
    }
    expose_one(vm, global, bank, CHECK_NAME, *CHECK.get().unwrap());
    expose_one(vm, global, bank, ASSERT_NAME, *ASSERT.get().unwrap());
}

/** The realm's bank offset selects its own materialized intrinsic, as the other conformance natives do. */
fn expose_one(vm: &mut Vm, global: ObjectId, bank: u32, name: &str, base: ObjectId) {
    let id = base + bank * banks::BANK_STRIDE;
    vm.materialize(id)
        .expect("installed failure trigger intrinsic");
    vm.heap
        .put(global, Key::from(name), Prop::data(JsValue::Object(id)));
}

/** A failed CHECK is never catchable by script: V8 aborts the process, so V9 ends the isolate with the failure. */
fn check_false(_: &mut Vm, _: Call) -> Res<JsValue> {
    Err(EngineError::new(
        CHECK_CODE,
        "Check failed: false.",
        "The upstream test deliberately triggered a CHECK failure.",
    )
    .into())
}

/** DCHECK is compiled out of release builds, so the assert trigger returns normally there. */
fn assert_false(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if cfg!(debug_assertions) {
        return check_false(vm, call);
    }
    Ok(JsValue::Undefined)
}
