/** Native window errors use intrinsic ErrorEvent identity and cancellation-aware default reporting. */
use super::*;
use crate::v9::vm::heap::{ENUMERABLE, Prop, Slot};

/** Native errors retain their prototype even if the author replaces the global constructor. */
static ERROR_PROTOTYPE: std::sync::OnceLock<ObjectId> = std::sync::OnceLock::new();

/** Register the deterministic intrinsic while installing the platform event constructors. */
pub(in crate::v9::platform) fn error_prototype(id: ObjectId) {
    let _ = ERROR_PROTOTYPE.set(id);
}

/** A resize-loop error is a trusted Window event with no thrown exception or source location. */
pub(in crate::v9) fn report_window_error(vm: &mut Vm, message: &str) -> Res<()> {
    let event = create_event(vm, "error", false, true)?;
    let id = event.object().expect("native error event");
    vm.pin(event);
    let proto = vm.intrinsic_id(*ERROR_PROTOTYPE.get().expect("error prototype installed"));
    vm.set_proto(id, Some(proto))?;
    for (name, value) in [
        ("message", JsValue::string(message)?),
        ("filename", JsValue::string("")?),
        ("lineno", JsValue::Number(0.0)),
        ("colno", JsValue::Number(0.0)),
        ("error", JsValue::Null),
    ] {
        vm.heap.put(
            id,
            Key::from(name),
            Prop {
                slot: Slot::Data(value),
                attrs: ENUMERABLE,
            },
        );
    }
    with_event(vm, id, |data| data.trusted = true);
    if dispatch(vm, Target::Window, id)? {
        vm.page.console("error", message.to_owned());
    }
    Ok(())
}

/** Plain Events named error keep the ordinary one-argument handler contract. */
pub(super) fn is_error_event(vm: &Vm, event: ObjectId) -> bool {
    let expected = vm.intrinsic_id(*ERROR_PROTOTYPE.get().expect("error prototype installed"));
    let mut current = Some(event);
    for _ in 0..crate::constants::VM_NESTING_DEPTH {
        let Some(id) = current else {
            return false;
        };
        if id == expected {
            return true;
        }
        current = vm.heap.get(id).and_then(|object| object.proto);
    }
    false
}
