/** test262's host hook `$262.evalScript`: run source as a new classic script of the page's realm, with
script declaration instantiation (global `var` and `function` bindings non-configurable, top-level
`let` and `const` in the script scope, collisions a SyntaxError), which an indirect eval cannot give. */
use super::*;
use crate::v9::value::ObjectId;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Prop};
use crate::v9::vm::realm::banks;
/** The native's banked identity; the page host exposes it under a name the harness prelude looks for. */
static NATIVE: OnceLock<ObjectId> = OnceLock::new();
/** The global the runner's prelude reads; hidden from enumeration like every host hook. */
const EXPOSED_NAME: &str = "__v9EvalScript";

/** Register the native once per realm build. */
pub(super) fn install(builder: &mut Builder) {
    let _ = NATIVE.set(builder.function("evalScript", 1, eval_script));
}

/** Put the hook on a page host's global; the d8 host has Realm.eval for the same job. */
pub(super) fn expose(vm: &mut Vm, global: ObjectId, bank: u32) {
    let id = *NATIVE.get().expect("installed evalScript intrinsic") + bank * banks::BANK_STRIDE;
    vm.materialize(id).expect("installed evalScript intrinsic");
    vm.heap.put(
        global,
        Key::from(EXPOSED_NAME),
        Prop {
            slot: crate::v9::vm::heap::Slot::Data(JsValue::Object(id)),
            attrs: HIDDEN_ATTRS,
        },
    );
}

/** Compile the argument as a script in the caller's realm and run it, returning its completion value. */
fn eval_script(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let source = vm.to_text(&call.arg(0))?;
    let bank = vm.object_realm(vm.global_object());
    let code = super::realms::script(vm, source, bank)?;
    vm.run_script(code)
}
