/** ShadowRealm.importValue uses the real module loader and copies only the chosen export across contexts. */
use super::*;
use crate::v9::vm::data::ReactionHandler;

/** Reactions are native closures whose captured global keeps a pending private context alive. */
static FULFILLED: OnceLock<u32> = OnceLock::new();
/** Rejection copying belongs to the caller's intrinsic bank. */
static REJECTED: OnceLock<u32> = OnceLock::new();
/** An outstanding outer capability retains its inner loader until either reaction completes. */
const INNER_SLOT: &str = "shadowImportInner";

/** The method shares the same real module graph and host request boundary as import(). */
pub(super) fn install(builder: &mut Builder, proto: ObjectId) {
    builder.method(proto, "importValue", 2, import_value);
    let _ = FULFILLED.set(builder.native("", 1, fulfilled));
    let _ = REJECTED.set(builder.native("", 1, rejected));
}

/** Conversion happens in the caller, and the module capability is created in the private context. */
fn import_value(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bank = context_for(vm, &call.this, "ShadowRealm.prototype.importValue")?;
    let specifier = vm.to_string(&call.arg(0))?;
    let export = vm.to_string(&call.arg(1))?;
    let site = vm.frames.last().map_or(0, |frame| frame.code.site);
    let imported = vm.with_realm(bank, |vm| {
        crate::v9::modules::dynamic_import(
            vm,
            JsValue::String(specifier.clone()),
            JsValue::Undefined,
            site,
        )
    })?;
    let output = vm.new_promise()?;
    vm.heap.put(
        output,
        Key::private(INNER_SLOT),
        Prop {
            slot: Slot::Data(imported.clone()),
            attrs: 0,
        },
    );
    let global = vm.realms.contexts[bank as usize]
        .global
        .expect("live context");
    let on_fulfilled = vm.native_closure(
        *FULFILLED.get().expect("installed"),
        vec![
            JsValue::String(specifier),
            JsValue::String(export),
            JsValue::Object(global),
            JsValue::Object(output),
        ],
    )?;
    let on_rejected = vm.native_closure(
        *REJECTED.get().expect("installed"),
        vec![JsValue::Object(output)],
    )?;
    vm.perform_then(
        imported.object().expect("module promise"),
        ReactionHandler::Function(JsValue::Object(on_fulfilled)),
        ReactionHandler::Function(JsValue::Object(on_rejected)),
        Some(output),
    )?;
    Ok(JsValue::Object(output))
}

/** Namespace lookup runs against its owner module map before the actual value is wrapped. */
fn fulfilled(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let slots = vm.slots(call.callee);
    vm.heap.remove(
        slots[3].object().expect("outer promise"),
        &Key::private(INNER_SLOT),
    );
    let bank = vm.object_realm(slots[2].object().expect("context global"));
    let JsValue::String(export) = &slots[1] else {
        unreachable!("native export slot")
    };
    let namespace = call.arg(0);
    let value = vm.with_realm(bank, |vm| {
        if vm
            .own(
                namespace.object().expect("module namespace"),
                &Key::Str(export.clone()),
            )?
            .is_none()
        {
            return Ok(None);
        }
        vm.get(&namespace, &Key::Str(export.clone())).map(Some)
    });
    match value {
        Ok(Some(value)) => wrap(vm, value),
        Ok(None) | Err(Thrown::Value(_)) => Err(missing_export(vm, &slots[0], &slots[1])),
        Err(fatal) => Err(fatal),
    }
}

/** Failed module loading or evaluation creates a caller-owned TypeError with copied stack data. */
fn rejected(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let output = vm.slots(call.callee)[0].object().expect("outer promise");
    vm.heap.remove(output, &Key::private(INNER_SLOT));
    Err(copy_error(
        vm,
        Thrown::Value(call.arg(0)),
        "Cannot import in ShadowRealm",
    ))
}

/** Export absence is distinguished from rejection of the module's own evaluation. */
fn missing_export(vm: &mut Vm, specifier: &JsValue, export: &JsValue) -> Thrown {
    let JsValue::String(specifier) = specifier else {
        unreachable!("specifier slot")
    };
    let JsValue::String(export) = export else {
        unreachable!("export slot")
    };
    vm.type_error(&format!(
        "The requested module '{}' does not provide an export named '{}'",
        specifier.lossy(),
        export.lossy()
    ))
}
