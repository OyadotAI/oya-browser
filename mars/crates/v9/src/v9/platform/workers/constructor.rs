/** Worker constructor uses only the current isolate and its branded handles. */
use super::*;

/** Validate construction synchronously; fetching remains an asynchronous coordinator task. */
pub(super) fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = call
        .new_target
        .as_ref()
        .ok_or_else(|| vm.type_error("Worker requires 'new'"))?;
    if call.args.is_empty() {
        return Err(vm.type_error("Worker requires a script URL"));
    }
    if vm.page.environment.worker_name.is_some()
        && vm.page.environment.worker_depth.max(1) >= WORKER_DEPTH
    {
        return Err(vm.unsupported_error("The dedicated worker nesting limit was reached"));
    }
    let text = vm.to_text(&call.arg(0))?;
    let base = url::Url::parse(&vm.page.environment.url)
        .map_err(|_| vm.dom_exception("SecurityError", "Invalid worker origin"))?;
    let url = base
        .join(&text)
        .map_err(|_| vm.dom_exception("SyntaxError", "Invalid worker URL"))?;
    if !matches!(url.scheme(), "http" | "https" | "blob") {
        return Err(vm.unsupported_error("This worker URL scheme is unavailable"));
    }
    if url.origin().ascii_serialization() != base.origin().ascii_serialization()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(vm.dom_exception("SecurityError", "The worker script must be same-origin"));
    }
    let options = call.arg(1);
    if !options.nullish() && !matches!(options, JsValue::Object(_)) {
        return Err(vm.type_error("Worker options must be a dictionary"));
    }
    let credentials = option(vm, &options, "credentials")?;
    let credentials = credentials_mode(vm, credentials)?;
    let name = option(vm, &options, "name")?;
    let name = if name == JsValue::Undefined {
        String::new()
    } else {
        vm.to_text(&name)?
    };
    let kind = if options.nullish() {
        JsValue::Undefined
    } else {
        vm.get(&options, &Key::from("type"))?
    };
    let worker_type = worker_type(vm, kind)?;
    if name.len() > crate::constants::URL_BYTES
        || url.as_str().len() > crate::constants::URL_BYTES
        || vm.page.state.workers.len() >= HANDLES
    {
        return Err(EngineError::limit("Worker handles", HANDLES).into());
    }
    capacity(vm)?;
    let id = u32::try_from(vm.page.state.workers.len() + 1).expect("bounded Worker handles");
    let command = WorkerCommand::Create {
        id,
        blob_source: super::super::blob_urls::resolve(&vm.page, url.as_str()),
        url: url.into(),
        name,
        worker_type,
        credentials,
    };
    let queued = vm
        .page
        .effects
        .worker_commands
        .iter()
        .try_fold(command.bytes()?, |total, item| {
            Ok::<_, EngineError>(total.saturating_add(item.bytes()?))
        })?;
    if queued > crate::v9::WORKER_MESSAGE_BYTES {
        return Err(
            EngineError::limit("Worker queued bytes", crate::v9::WORKER_MESSAGE_BYTES).into(),
        );
    }
    let prototype = vm.prototype_for(target, *PROTOTYPE.get().expect("installed Worker"))?;
    let object = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    vm.page.state.workers.insert(
        id,
        Handle {
            object,
            active: true,
        },
    );
    vm.page.effects.worker_commands.push(command);
    Ok(JsValue::Object(object))
}

/** Constructor enum conversion does not accidentally treat arbitrary values as module mode. */
fn worker_type(vm: &mut Vm, value: JsValue) -> Res<WorkerType> {
    if value == JsValue::Undefined {
        return Ok(WorkerType::Classic);
    }
    match vm.to_text(&value)?.as_str() {
        "classic" => Ok(WorkerType::Classic),
        "module" => Ok(WorkerType::Module),
        _ => Err(vm.type_error("Invalid Worker type")),
    }
}

/** Retain valid credentials even though classic workers deliberately ignore this option. */
fn credentials_mode(vm: &mut Vm, value: JsValue) -> Res<WorkerCredentials> {
    if value == JsValue::Undefined {
        return Ok(WorkerCredentials::SameOrigin);
    }
    match vm.to_text(&value)?.as_str() {
        "omit" => Ok(WorkerCredentials::Omit),
        "same-origin" => Ok(WorkerCredentials::SameOrigin),
        "include" => Ok(WorkerCredentials::Include),
        _ => Err(vm.type_error("Invalid Worker credentials")),
    }
}

/** Dictionary members are read in WebIDL order even when classic loading ignores credentials. */
fn option(vm: &mut Vm, options: &JsValue, name: &str) -> Res<JsValue> {
    if options.nullish() {
        Ok(JsValue::Undefined)
    } else {
        vm.get(options, &Key::from(name))
    }
}
