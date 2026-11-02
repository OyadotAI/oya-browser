/** Synchronous classic imports retain the running worker stack while the owner supplies source. */
use crate::constants::{SCRIPT_BYTES, SCRIPT_TOTAL_BYTES, URL_BYTES};
use crate::protocol::EngineError;
use crate::v9::vm::{Call, DynamicSource, Res, Thrown, Vm};
use crate::v9::{builtins, compile_dynamic, value::JsValue};

/** Callback and nesting limits bound host work even when imported scripts recursively import. */
pub(in crate::v9) const IMPORTS: usize = 32;
const DEPTH: usize = 16;

/** Convert every URL before acquisition, then execute imports in order without microtask pumping. */
pub(super) fn run(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if vm.page.environment.worker_name.is_none() {
        return Err(vm.type_error("Illegal invocation"));
    }
    if vm.page.environment.worker_module {
        return Err(vm.type_error("Module workers do not support importScripts"));
    }
    if call.args.len() > IMPORTS || vm.page.import_depth >= DEPTH {
        return Err(EngineError::limit("worker imports", IMPORTS).into());
    }
    let base = url::Url::parse(&vm.page.environment.url)
        .map_err(|_| vm.dom_exception("SyntaxError", "Invalid worker script URL"))?;
    let mut urls = Vec::new();
    for argument in call.args {
        let text = vm.to_text(argument)?;
        let url = base
            .join(&text)
            .map_err(|_| vm.dom_exception("SyntaxError", "Invalid imported script URL"))?;
        if url.as_str().len() > URL_BYTES {
            return Err(EngineError::limit("worker import URL bytes", URL_BYTES).into());
        }
        urls.push(url.to_string());
    }
    vm.page.import_depth += 1;
    let result = urls.iter().try_for_each(|url| execute(vm, url));
    vm.page.import_depth -= 1;
    result.map(|()| JsValue::Undefined)
}

/** A host error cannot be swallowed; ordinary loading refusal is a catchable NetworkError. */
fn source(vm: &mut Vm, url: &str) -> Res<String> {
    if vm.page.import_count >= IMPORTS {
        return Err(EngineError::limit("worker import callbacks", IMPORTS).into());
    }
    vm.page.import_count += 1;
    let host = vm
        .page
        .imports
        .ok_or_else(|| vm.unsupported_error("Worker script loading is unavailable"))?;
    let source = host
        .load(url)?
        .ok_or_else(|| vm.dom_exception("NetworkError", "The worker script could not be loaded"))?;
    vm.page.import_bytes = vm.page.import_bytes.saturating_add(source.len());
    if source.len() > SCRIPT_BYTES || vm.page.import_bytes > SCRIPT_TOTAL_BYTES {
        return Err(EngineError::limit("worker imported source bytes", SCRIPT_TOTAL_BYTES).into());
    }
    Ok(source)
}

/** Compile a global Script, retain its code for future callbacks, and execute on this same VM. */
fn execute(vm: &mut Vm, url: &str) -> Res<()> {
    let source = source(vm, url)?;
    vm.prepare_dynamic(source.len())?;
    let base = builtins::dynamic_base(&vm.dynamic);
    let code = compile_dynamic(&mut vm.registry, &source, base, None, false)
        .map_err(|error| syntax(vm, error))?;
    if vm.page.state.worker_imports.len() >= IMPORTS {
        return Err(EngineError::limit("worker retained imports", IMPORTS).into());
    }
    vm.page.state.worker_imports.insert(base, url.into());
    vm.dynamic.push(DynamicSource {
        source,
        context: None,
        base: Some(base),
        origin: None,
    });
    vm.run_script(code).map(|_| ())
}

/** Imported syntax failures remain JavaScript exceptions, while native limits remain fatal. */
fn syntax(vm: &mut Vm, error: EngineError) -> Thrown {
    if error.code == "script_syntax_error" {
        vm.syntax_error(error.message.trim_start_matches("SyntaxError: "))
    } else {
        error.into()
    }
}
