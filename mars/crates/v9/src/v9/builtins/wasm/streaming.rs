/** The opt-in d8 streaming callback consumes promised native byte sources through real Promise jobs. */
use super::*;
use crate::v9::vm::data::ReactionHandler;

/** Public methods use one native continuation, retaining only inert options and traced import values. */
pub(super) fn install(b: &mut Builder, namespace: ObjectId) -> u32 {
    b.method(namespace, "compileStreaming", 1, compile);
    b.method(namespace, "instantiateStreaming", 1, instantiate);
    b.native("", 1, received)
}
/** Copy already evaluated options into a private record without replaying any authored getter. */
fn options_record(vm: &mut Vm, value: &JsValue) -> Res<ObjectId> {
    let options = imports::options(vm, value)?;
    let record = vm.new_object()?;
    imports::store(vm, record, options)?;
    Ok(record)
}
/** Standard Response streaming requires an embedding-owned source consumer, never implicit network IO. */
fn allowed(vm: &Vm) -> bool {
    #[cfg(feature = "conformance")]
    {
        vm.page.environment.conformance_d8
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        false
    }
}
/** The returned promise belongs to the intrinsic constructor even when user Promise species changes. */
fn begin(
    vm: &mut Vm,
    source: JsValue,
    options: JsValue,
    imports: JsValue,
    instantiate: bool,
) -> Res<JsValue> {
    let result = vm.new_promise()?;
    let prepared = (|| {
        if !allowed(vm) {
            return Err(vm.unsupported_error("WebAssembly Response streaming"));
        }
        let options = options_record(vm, &options)?;
        let input = vm.new_promise()?;
        vm.resolve_promise(input, source)?;
        let callback = vm.native_closure(
            ids(vm).stream_received,
            vec![
                JsValue::Object(options),
                imports,
                JsValue::Boolean(instantiate),
            ],
        )?;
        vm.perform_then(
            input,
            ReactionHandler::Function(JsValue::Object(callback)),
            ReactionHandler::Function(JsValue::Undefined),
            Some(result),
        )
    })();
    match prepared {
        Ok(()) => (),
        Err(Thrown::Value(error)) => vm.settle(result, PromiseState::Rejected(error))?,
        Err(fatal) => return Err(fatal),
    }
    Ok(JsValue::Object(result))
}
/** Compilation starts only after the source Promise has fulfilled with its actual BufferSource. */
fn compile(vm: &mut Vm, call: Call) -> Res<JsValue> {
    begin(vm, call.arg(0), call.arg(1), JsValue::Undefined, false)
}
/** Imports and compile options remain separately ordered, as with the byte instantiate overload. */
fn instantiate(vm: &mut Vm, call: Call) -> Res<JsValue> {
    begin(vm, call.arg(0), call.arg(2), call.arg(1), true)
}
/** A fulfilled source is copied and genuinely compiled; failure rejects the actual output Promise. */
fn received(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let result = (|| {
        let slots = vm.slots(call.callee);
        let record = slots
            .first()
            .and_then(|v| v.object())
            .ok_or_else(|| vm.type_error("invalid Wasm streaming continuation"))?;
        let source = bytes(vm, &call.arg(0))?;
        compile_limits::check(vm, source.len(), true)?;
        vm.charge(source.len())?;
        let (selected, namespace) = imports::snapshot(vm, record);
        let selected = selected
            .iter()
            .map(|s| JsValue::string(s))
            .collect::<crate::protocol::EngineResult<Vec<_>>>()?;
        let namespace = namespace
            .map(|units| JsString::from_units(units).map(JsValue::String))
            .transpose()?;
        let module = finish_module(vm, source, (selected, namespace), None)?;
        if slots.get(2) != Some(&JsValue::Boolean(true)) {
            return Ok(JsValue::Object(module));
        }
        let instance = instance::create(vm, &JsValue::Object(module), &slots[1], None)?;
        let result = vm.new_object()?;
        vm.put(result, "module", JsValue::Object(module))?;
        vm.put(result, "instance", JsValue::Object(instance))?;
        Ok(JsValue::Object(result))
    })();
    let api = if vm.slots(call.callee).get(2) == Some(&JsValue::Boolean(true)) {
        "WebAssembly.instantiateStreaming()"
    } else {
        "WebAssembly.compileStreaming()"
    };
    result.map_err(|e| api_prefix(vm, e, api))
}
