/** Class auto-accessors use branded private backing fields and real getter/setter function identities. */
use super::*;
use crate::protocol::EngineError;
use crate::v9::vm::data::FunctionName;
use std::sync::OnceLock;

/** Getter and setter entry points have distinct observable lengths and are never constructors. */
static NATIVES: OnceLock<(u32, u32)> = OnceLock::new();

/** Internal accessor factories add no authored global or overridable constructor dependency. */
pub(super) fn install(builder: &mut Builder) {
    let _ = NATIVES.set((builder.native("", 0, get), builder.native("", 1, set)));
}

/** Each accessor pair captures the backing key minted by this particular class evaluation. */
pub(in crate::v9) fn create(vm: &mut Vm, public: &Key, backing: Key) -> Res<(ObjectId, ObjectId)> {
    let Key::Private(backing) = backing else {
        return Err(EngineError::invalid("Auto-accessor backing must be private.").into());
    };
    let (get, set) = *NATIVES.get().expect("auto-accessors installed");
    let getter = vm.native_closure(get, vec![JsValue::String(backing.clone())])?;
    vm.pin(JsValue::Object(getter));
    let setter = vm.native_closure(set, vec![JsValue::String(backing)])?;
    vm.pin(JsValue::Object(setter));
    let name = public_name(vm, public)?;
    for (function, prefix) in [(getter, "get "), (setter, "set ")] {
        let name = JsString::from_utf8(prefix)?.concat(&name)?;
        vm.set_name_snapshot(function, FunctionName::flat(JsValue::String(name)))?;
    }
    Ok((getter, setter))
}

/** Property names retain UTF-16 code units; symbol names use the SetFunctionName bracket form. */
pub(in crate::v9) fn public_name(vm: &Vm, key: &Key) -> Res<JsString> {
    Ok(match key {
        Key::Str(name) => name.clone(),
        Key::Private(name) => JsString::from_utf8(&format!(
            "#{}",
            name.lossy().rsplit(':').next().unwrap_or("")
        ))?,
        Key::Sym(id) => match super::symbol::symbol_description(vm, *id) {
            Some(description) => JsString::from_utf8(&format!("[{description}]"))?,
            None => JsString::empty(),
        },
    })
}

/** Only directly captured private keys are admitted; forged state never falls back to public property lookup. */
fn backing(vm: &Vm, callee: ObjectId) -> Res<Key> {
    match vm.slots(callee).as_slice() {
        [JsValue::String(name)] => Ok(Key::Private(name.clone())),
        _ => Err(EngineError::invalid("Invalid auto-accessor capture.").into()),
    }
}

/** The getter performs the same own-brand check as an authored private field read. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let key = backing(vm, call.callee)?;
    vm.private_brand(&call.this, &key, false)?;
    vm.get(&call.this, &key)
}

/** The setter mutates its private backing even when the instance's public properties are frozen. */
fn set(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let key = backing(vm, call.callee)?;
    vm.private_brand(&call.this, &key, true)?;
    vm.set(&call.this, key, call.arg(0), true)?;
    Ok(JsValue::Undefined)
}
