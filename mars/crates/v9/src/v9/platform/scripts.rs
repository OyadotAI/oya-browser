/**
 * Classic scripts a page inserts after it loaded: a `<script src>` element connected by script
 * asks the app for its source (`Execution::script_requests`) and runs once when the next
 * `load_modules` step delivers it as a classic `ModuleSource`; an inline `<script>` connected by
 * script runs at once as dynamic code. This is how Google's and YouTube's frameworks load the
 * code behind a click, so without it a button does nothing.
 */
use super::{Page, ScriptLoad, Target, fire};
use crate::constants::MODULES;
use crate::protocol::EngineResult;
use crate::v9::value::JsValue;
use crate::v9::vm::{Res, Thrown, Vm};

/** JavaScript media types a `type` attribute may name and still be a classic script. */
const CLASSIC_TYPES: [&str; 8] = [
    "text/javascript",
    "application/javascript",
    "text/ecmascript",
    "application/ecmascript",
    "text/jscript",
    "application/x-javascript",
    "text/x-javascript",
    "module",
];

/** Whether a `type` (or `language`) attribute names a classic script. */
fn classic(attributes: &std::collections::BTreeMap<String, String>) -> bool {
    match attributes
        .get("type")
        .map(|kind| kind.trim().to_ascii_lowercase())
    {
        None => true,
        Some(kind) if kind.is_empty() => true,
        // ponytail: a module script inserted by script waits for module loading through the
        // page's module map; treat it as not classic so it is skipped rather than run wrongly.
        Some(kind) => kind != "module" && CLASSIC_TYPES.contains(&kind.as_str()),
    }
}

/** Record that a script element started, so it never runs twice; false when it had started. */
fn start(
    page: &mut Page,
    node: usize,
    url: String,
    attributes: &std::collections::BTreeMap<String, String>,
) -> bool {
    if page
        .state
        .script_loads
        .iter()
        .any(|load| load.element == node)
    {
        return false;
    }
    if page.state.script_loads.len() >= MODULES {
        return false;
    }
    page.state.script_loads.push(ScriptLoad {
        url,
        element: node,
        nonce: attributes.get("nonce").cloned(),
        integrity: attributes.get("integrity").cloned(),
        cross_origin: attributes.get("crossorigin").cloned(),
        done: false,
    });
    true
}

/** A node (and its descendants) just connected, or a connected script's `src` just set: start
every classic script among them that has not started. */
pub(in crate::v9) fn connected(vm: &mut Vm, node: usize) -> Res<()> {
    if !vm.page.connected(node) {
        return Ok(());
    }
    let mut nodes = vec![node];
    nodes.extend(vm.page.descendants(node));
    for node in nodes {
        if vm.page.tag(node) == "script" {
            connected_script(vm, node)?;
        }
    }
    Ok(())
}

/** Start one connected script element: request its source, or run its inline text. */
fn connected_script(vm: &mut Vm, node: usize) -> Res<()> {
    let Some(view) = vm.page.view(node) else {
        return Ok(());
    };
    if !classic(&view.attributes) {
        return Ok(());
    }
    if let Some(src) = view
        .attributes
        .get("src")
        .filter(|src| !src.trim().is_empty())
    {
        let Some(url) = super::net::resolve(vm, src) else {
            return Ok(());
        };
        start(&mut vm.page, node, url, &view.attributes);
        return Ok(());
    }
    let source = vm.page.text_content(node);
    if source.trim().is_empty() || !start(&mut vm.page, node, String::new(), &view.attributes) {
        return Ok(());
    }
    vm.page
        .state
        .script_loads
        .last_mut()
        .expect("just pushed")
        .done = true;
    let previous = vm.page.current_script.replace(node);
    let result = crate::v9::builtins::evaluate_source(vm, source).and_then(|_| vm.drain_jobs());
    vm.page.current_script = previous;
    quiet(vm, result.map(|()| JsValue::Undefined))
}

/** An uncaught exception in an inserted script reaches the console, never the inserter. */
fn quiet(vm: &mut Vm, result: Res<JsValue>) -> Res<()> {
    match result {
        Ok(_) => Ok(()),
        Err(Thrown::Value(error)) => {
            vm.report_error(&error);
            Ok(())
        }
        Err(fatal) => Err(fatal),
    }
}

/** Run every started `<script src>` whose source this request delivered, in start order; the
element then gets its `load` event, or `error` when the app refused or could not fetch it. */
pub(in crate::v9) fn run_delivered(vm: &mut Vm) -> EngineResult<()> {
    for index in 0..vm.page.state.script_loads.len() {
        let load = vm.page.state.script_loads[index].clone();
        if load.done || load.url.is_empty() {
            continue;
        }
        let Some(code) = vm.modules.loaded(&load.url) else {
            continue;
        };
        vm.page.state.script_loads[index].done = true;
        let kind = match code {
            Ok(code) => {
                vm.remaining = vm.limits.operations;
                vm.page.throw_location = None;
                let previous = vm.page.current_script.replace(load.element);
                let result = vm
                    .run_script(code)
                    .and_then(|_| vm.drain_jobs().map(|()| JsValue::Undefined));
                vm.page.current_script = previous;
                quiet(vm, result).map_err(fatal)?;
                "load"
            }
            Err(_) => "error",
        };
        if vm.page.exists(load.element) {
            let fired = fire(vm, Target::Node(load.element), kind, false)
                .and_then(|_| vm.drain_jobs().map(|()| JsValue::Undefined));
            quiet(vm, fired).map_err(fatal)?;
        }
    }
    Ok(())
}

/** A fatal throw as the engine error it carries. */
fn fatal(thrown: Thrown) -> crate::protocol::EngineError {
    match thrown {
        Thrown::Fatal(error) => error,
        Thrown::Value(_) => crate::protocol::EngineError::invalid("A reported error was fatal."),
    }
}

/** Follow a script element to its new identity after the document adopted it. */
pub(in crate::v9) fn adopt(page: &mut Page, old: usize, new: usize) {
    for load in &mut page.state.script_loads {
        if load.element == old {
            load.element = new;
        }
    }
}

/** Starting a script never fails the insertion that connected it: its errors go to the console. */
pub(in crate::v9) fn after_insert(vm: &mut Vm, node: usize) -> Res<()> {
    match connected(vm, node) {
        Ok(()) => Ok(()),
        Err(Thrown::Value(error)) => {
            vm.report_error(&error);
            Ok(())
        }
        Err(fatal) => Err(fatal),
    }
}
