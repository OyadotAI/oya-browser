/** Async instantiation adopts the actual compiled Module Promise before linking its resolved value. */
use super::*;
use crate::v9::vm::data::ReactionHandler;
/** A receiver callback is captured as an ordinary native closure and participates in normal tracing/jobs. */
pub(super) fn install(builder: &mut Builder) -> u32 {
    builder.native("", 1, received)
}
/** Bytes compile into a genuine Promise chain; the existing-Module overload keeps its Instance result shape. */
pub(super) fn instantiate(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call
        .arg(0)
        .object()
        .is_some_and(|id| get(vm, id, MODULE).is_some())
    {
        let result = instance::create(vm, &call.arg(0), &call.arg(1), None).map(JsValue::Object);
        let result = result.map_err(|error| api_prefix(vm, error, "WebAssembly.instantiate()"));
        return promised(vm, result);
    }
    let output = vm.new_promise()?;
    let result = (|| {
        let compiled = vm.new_promise()?;
        let module = make_module(vm, &call.arg(0), &call.arg(2), None)
            .map(JsValue::Object)
            .map_err(|error| api_prefix(vm, error, "WebAssembly.instantiate()"));
        match module {
            Ok(module) => vm.resolve_promise(compiled, module)?,
            Err(Thrown::Value(error)) => vm.settle(compiled, PromiseState::Rejected(error))?,
            Err(fatal) => return Err(fatal),
        }
        let callback = vm.native_closure(ids(vm).instantiate_received, vec![call.arg(1)])?;
        vm.perform_then(
            compiled,
            ReactionHandler::Function(JsValue::Object(callback)),
            ReactionHandler::Function(JsValue::Undefined),
            Some(output),
        )
    })();
    match result {
        Ok(()) => (),
        Err(Thrown::Value(error)) => vm.settle(output, PromiseState::Rejected(error))?,
        Err(fatal) => return Err(fatal),
    }
    Ok(JsValue::Object(output))
}
/** Promise adoption may return an authored different value, so the actual callback rechecks the Module brand. */
fn received(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let module = call.arg(0);
    if !module
        .object()
        .is_some_and(|id| get(vm, id, MODULE).is_some())
    {
        return Err(vm.type_error(
            "Illegal call to WebAssembly.instantiate callback: Argument is not a WasmModuleObject",
        ));
    }
    let imports = vm
        .slots(call.callee)
        .first()
        .cloned()
        .ok_or_else(|| vm.type_error("invalid async WebAssembly imports capture"))?;
    let instance = instance::create(vm, &module, &imports, None)
        .map_err(|error| api_prefix(vm, error, "WebAssembly.instantiate()"))?;
    let result = vm.new_object()?;
    vm.define(result, key("module"), Descriptor::data(module))?;
    vm.define(
        result,
        key("instance"),
        Descriptor::data(JsValue::Object(instance)),
    )?;
    Ok(JsValue::Object(result))
}
