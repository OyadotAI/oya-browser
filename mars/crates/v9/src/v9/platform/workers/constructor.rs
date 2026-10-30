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
    let text = vm.to_text(&call.arg(0))?;
    let base = url::Url::parse(&vm.page.environment.url)
        .map_err(|_| vm.dom_exception("SecurityError", "Invalid worker origin"))?;
    let url = base
        .join(&text)
        .map_err(|_| vm.dom_exception("SyntaxError", "Invalid worker URL"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(vm.unsupported_error("This worker URL scheme is unavailable"));
    }
    if url.origin() != base.origin() || !url.username().is_empty() || url.password().is_some() {
        return Err(vm.dom_exception("SecurityError", "The worker script must be same-origin"));
    }
    let options = call.arg(1);
    if !options.nullish() && !matches!(options, JsValue::Object(_)) {
        return Err(vm.type_error("Worker options must be a dictionary"));
    }
    let credentials = option(vm, &options, "credentials")?;
    if credentials != JsValue::Undefined
        && !matches!(
            vm.to_text(&credentials)?.as_str(),
            "omit" | "same-origin" | "include"
        )
    {
        return Err(vm.type_error("Invalid Worker credentials"));
    }
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
    if kind != JsValue::Undefined {
        match vm.to_text(&kind)?.as_str() {
            "classic" => {}
            "module" => return Err(vm.unsupported_error("Module workers are unavailable")),
            _ => return Err(vm.type_error("Invalid Worker type")),
        }
    }
    if name.len() > crate::constants::URL_BYTES
        || url.as_str().len() > crate::constants::URL_BYTES
        || vm.page.state.workers.len() >= HANDLES
    {
        return Err(EngineError::limit("Worker handles", HANDLES).into());
    }
    capacity(vm)?;
    let id = u32::try_from(vm.page.state.workers.len() + 1).expect("bounded Worker handles");
    let prototype = vm.prototype_for(target, *PROTOTYPE.get().expect("installed Worker"))?;
    let object = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    vm.page.state.workers.insert(
        id,
        Handle {
            object,
            active: true,
        },
    );
    vm.page.effects.worker_commands.push(WorkerCommand::Create {
        id,
        url: url.into(),
        name,
    });
    Ok(JsValue::Object(object))
}

/** Dictionary members are read in WebIDL order even when classic loading ignores credentials. */
fn option(vm: &mut Vm, options: &JsValue, name: &str) -> Res<JsValue> {
    if options.nullish() {
        Ok(JsValue::Undefined)
    } else {
        vm.get(options, &Key::from(name))
    }
}
