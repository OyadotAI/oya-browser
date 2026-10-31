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
/** The MIME type a Response must carry to be compiled, as WebAssembly's Web API requires. */
const WASM_MIME: &str = "application/wasm";

/** A fulfilled source that is a Response: check it as the Web API's "process a Response" step
does, then read its bytes through its own `arrayBuffer()` and continue with them. None when the
source is not a Response, so the caller compiles it as a byte source. */
fn response_bytes(vm: &mut Vm, source: &JsValue, continuation: ObjectId) -> Res<Option<JsValue>> {
    let reader = match source {
        JsValue::Object(_) => vm.get(source, &Key::from("arrayBuffer"))?,
        _ => return Ok(None),
    };
    if !vm.is_callable(&reader) || vm.get(source, &Key::from("headers"))?.object().is_none() {
        return Ok(None);
    }
    check_response(vm, source)?;
    let promise = vm.call(reader, source.clone(), Vec::new())?;
    let Some(read) = promise.object() else {
        return Err(vm.type_error("Response.arrayBuffer() did not return a promise"));
    };
    let next = vm.new_promise()?;
    vm.perform_then(
        read,
        ReactionHandler::Function(JsValue::Object(continuation)),
        ReactionHandler::Function(JsValue::Undefined),
        Some(next),
    )?;
    Ok(Some(JsValue::Object(next)))
}

/** A Response streams only when it is OK, unread and served as application/wasm. */
fn check_response(vm: &mut Vm, response: &JsValue) -> Res<()> {
    let headers = vm.get(response, &Key::from("headers"))?;
    let get = vm.get(&headers, &Key::from("get"))?;
    let kind = vm.call(get, headers, vec![JsValue::string("content-type")?])?;
    let essence = if kind.nullish() {
        String::new()
    } else {
        vm.to_text(&kind)?
    };
    let essence = essence
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if essence != WASM_MIME {
        return Err(vm.type_error("Incorrect response MIME type. Expected 'application/wasm'."));
    }
    if vm.get(response, &Key::from("ok"))? != JsValue::Boolean(true) {
        return Err(vm.type_error("HTTP status code is not ok"));
    }
    if vm.get(response, &Key::from("bodyUsed"))? == JsValue::Boolean(true) {
        return Err(
            vm.type_error("Cannot compile WebAssembly.Module from an already read Response")
        );
    }
    Ok(())
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
    if let Some(next) = response_bytes(vm, &call.arg(0), call.callee)
        .map_err(|e| api_prefix(vm, e, "WebAssembly.compileStreaming()"))?
    {
        return Ok(next);
    }
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
