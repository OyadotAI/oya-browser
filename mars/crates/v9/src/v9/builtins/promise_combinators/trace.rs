/** Async trace metadata comes from the actual native Promise.all element closure and capability, never a function-name guess. */
use super::*;
use crate::v9::vm::data::FunctionKind;
use crate::v9::vm::heap::Slot;
use crate::v9::vm::realm::Static;

/** The exact intrinsic and output promise associated with one pending Promise.all element. */
pub(in crate::v9) struct AllTrace {
    /** Immutable native method identity from the element closure's creation context. */
    pub function: ObjectId,
    /** That intrinsic's Promise constructor is the combinator CallSite receiver. */
    pub receiver: ObjectId,
    /** Awaiters of this output promise continue the async ancestry. */
    pub promise: ObjectId,
    /** Actual captured input position, bounded by the engine's resident element limit. */
    pub index: u32,
}

/** Native callbacks passed to overridden then methods are opaque; inspect only the exact owned pending closure. */
pub(in crate::v9) fn all_trace(vm: &Vm, handler: &JsValue) -> Option<AllTrace> {
    let id = handler.object()?;
    let Class::Function(function) = &vm.heap.get(id)?.class else {
        return None;
    };
    let FunctionKind::Native { id: native, slots } = &function.kind else {
        return None;
    };
    if *native != natives().all_element {
        return None;
    }
    let [
        JsValue::Number(index),
        JsValue::Object(values),
        resolve,
        JsValue::Object(remaining),
        JsValue::Object(called),
    ] = slots.as_slice()
    else {
        return None;
    };
    if !index.is_finite()
        || *index < 0.0
        || index.fract() != 0.0
        || *index >= vm.limits.array_length as f64
    {
        return None;
    }
    let Class::Array(values) = &vm.heap.get(*values)?.class else {
        return None;
    };
    if *index as usize >= values.len() || !pending(vm, *called) || !remaining_count(vm, *remaining)
    {
        return None;
    }
    let promise = super::super::promise::resolving_promise(vm, resolve)?;
    let intrinsics = vm.object_intrinsics(id);
    let receiver = intrinsics.ids.promise;
    let (_, Static::Value(JsValue::Object(function)), _) =
        intrinsics.object(receiver)?.get(&key("all"))?
    else {
        return None;
    };
    Some(AllTrace {
        function: *function,
        receiver,
        promise,
        index: *index as u32,
    })
}

/** Read the closure's private plain flag without invoking any property getter. */
fn pending(vm: &Vm, id: ObjectId) -> bool {
    vm.heap.get(id).is_some_and(|object| {
        matches!(object.class, Class::Plain)
            && object.props.get(&key("done")).is_none_or(|prop| {
                matches!(
                    &prop.slot,
                    Slot::Data(JsValue::Boolean(false) | JsValue::Undefined)
                )
            })
    })
}

/** An already-completed aggregate no longer supplies a suspended combinator continuation. */
fn remaining_count(vm: &Vm, id: ObjectId) -> bool {
    vm.heap.get(id).is_some_and(|object| {
        matches!(object.class, Class::Plain)
            && object.props.get(&key("n")).is_some_and(
                |prop| matches!(&prop.slot,Slot::Data(JsValue::Number(n))if n.is_finite()&&*n>0.0),
            )
    })
}
