/** Native map entry captures immutable native metadata and actual identities without inventing a source location. */
use super::*;
use crate::v9::vm::{NativeActivation, NativeKind, data::FunctionKind};

impl Captured {
    /** Private identities remain real even though strict builtin frames hide them from public accessors. */
    pub(super) fn native(vm: &Vm, activation: &NativeActivation, _strict: bool) -> Self {
        let (callee, receiver, strict, construct) = match &activation.kind {
            NativeKind::Wasm {
                instance,
                index,
                position,
            } => return Self::wasm(activation.realm, *instance, *index, *position),
            NativeKind::Builtin {
                callee,
                receiver,
                construct,
            } => (callee, receiver, true, *construct),
            #[cfg(feature = "conformance")]
            NativeKind::Api { callee, receiver } => (callee, receiver, _strict, false),
        };
        let intrinsics = vm.object_intrinsics(*callee);
        let definition = intrinsics.native(*callee).or_else(|| {
            let function = vm.heap.get(*callee)?.function()?;
            let FunctionKind::Native { id, .. } = &function.kind else {
                return None;
            };
            intrinsics.natives.get(*id as usize)
        });
        Self {
            function: JsValue::Object(*callee),
            receiver: receiver.clone(),
            name: JsString::from_utf8(definition.map_or("", |native| native.name))
                .expect("bounded native name"),
            file: String::new(),
            line: 0,
            column: 0,
            position: 0,
            realm: activation.realm,
            strict,
            construct,
            eval: false,
            site: 0,
            asynchronous: false,
            native: false,
            builtin: true,
            wasm: false,
            promise_index: None,
        }
    }

    /** Wasm captures retain the real numeric function index and instance, with no JS strict-mode flag. */
    fn wasm(realm: u32, instance: ObjectId, index: u32, position: usize) -> Self {
        Self {
            function: JsValue::Number(index as f64),
            receiver: JsValue::Object(instance),
            name: JsString::empty(),
            file: String::new(),
            line: 1,
            column: position as u32 + 1,
            position: position as u32,
            realm,
            strict: false,
            construct: false,
            eval: false,
            site: 0,
            asynchronous: false,
            native: false,
            builtin: false,
            wasm: true,
            promise_index: None,
        }
    }
}
