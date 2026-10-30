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
        Some(kind) => mars_core::http::is_javascript_mime_essence(&kind),
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
        match vm.page.tag(node).as_str() {
            "script" => connected_script(vm, node)?,
            "link" => connected_sheet(vm, node)?,
            "iframe" => super::frames::connected(vm, node)?,
            _ => {}
        }
    }
    Ok(())
}

/// The native that fires a stylesheet link's `load` event, made once with the realm.
static SHEET_LOADED: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

/** Make the stylesheet `load` native; runs once, inside the realm build. */
pub(super) fn install(b: &mut crate::v9::vm::realm::Builder) {
    let _ = SHEET_LOADED.set(b.function("", 1, sheet_loaded));
}

/**
 * A `<link rel=stylesheet href>` connected by script gets its `load` event in a later task, as
 * a browser fires it once the sheet arrives; the app fetches and applies the sheet itself.
 * Loaders that wait on it (craigslist keeps its curtain up until its CSS reports loaded) carry
 * on. ponytail: `load` even when the fetch fails, where a browser fires `error`; tell them
 * apart once V9 hears back about sheets.
 */
fn connected_sheet(vm: &mut Vm, node: usize) -> Res<()> {
    let Some(href) = sheet_href(vm, node) else {
        return Ok(());
    };
    request_sheet(vm, href);
    schedule_load(vm, node)
}

/** The resolved `href` of a `<link rel=stylesheet>`, or None for any other node. */
fn sheet_href(vm: &mut Vm, node: usize) -> Option<String> {
    let view = vm.page.view(node).filter(|view| view.tag == "link")?;
    let sheet = view.attributes.get("rel").is_some_and(|rel| {
        rel.split_ascii_whitespace()
            .any(|token| token.eq_ignore_ascii_case("stylesheet"))
    });
    let href = view
        .attributes
        .get("href")
        .filter(|href| !href.trim().is_empty())?;
    sheet.then(|| super::net::resolve(vm, href)).flatten()
}

/** Ask the app to fetch and apply a sheet, once per address and within the module bound. */
fn request_sheet(vm: &mut Vm, href: String) {
    if vm.page.effects.stylesheets.len() < MODULES && !vm.page.effects.stylesheets.contains(&href) {
        vm.page.effects.stylesheets.push(href);
    }
}

/** Queue the link's `load` event as a later task, as a browser fires it once the sheet arrives. */
fn schedule_load(vm: &mut Vm, node: usize) -> Res<()> {
    let Some(native) = SHEET_LOADED.get().copied() else {
        return Ok(());
    };
    let link = super::dom::wrap(vm, node)?;
    super::timers::schedule(vm, JsValue::Object(native), vec![link])
}

/**
 * Parsed `<link rel=stylesheet>` elements were fetched by navigation; each still gets its `load`
 * event in a later task, after the parser's scripts, as Blink fires it when the sheet arrives.
 * Pages defer a sheet as `media=print` and set `media='all'` from that `load` (google.com shows
 * its Gmail and Images links this way). Runs once per fresh load, before its scripts.
 * scripts.
 */
pub(in crate::v9) fn parsed_sheets(vm: &mut Vm) -> Res<()> {
    let mut nodes = vm.page.descendants(super::HEAD);
    nodes.extend(vm.page.descendants(super::BODY));
    for node in nodes {
        if sheet_href(vm, node).is_some() {
            schedule_load(vm, node)?;
        }
    }
    Ok(())
}

/**
 * A connected sheet link's `media` changed: once it holds, the sheet applies, as Blink
 * re-evaluates a link's media list on mutation. ponytail: the app appends the sheet after the
 * others rather than at the link's place in the cascade, and a sheet whose media stops holding
 * keeps applying; track sheets per link when a page needs either.
 */
pub(in crate::v9) fn media_changed(vm: &mut Vm, node: usize) {
    let media = vm
        .page
        .view(node)
        .and_then(|view| view.attributes.get("media").cloned())
        .unwrap_or_default();
    if !vm.page.connected(node) || !super::window::media_matches(vm, &media) {
        return;
    }
    if let Some(href) = sheet_href(vm, node) {
        request_sheet(vm, href);
    }
}

/** Fire `load` on the link a sheet task names, if it is still in the page. */
fn sheet_loaded(vm: &mut Vm, call: crate::v9::vm::Call) -> Res<JsValue> {
    if let Some(node) = super::dom::node_of(vm, &call.arg(0))
        && vm.page.exists(node)
    {
        fire(vm, Target::Node(node), "load", false)?;
    }
    Ok(JsValue::Undefined)
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
        completed(vm, load.element, kind)?;
    }
    Ok(())
}

/** Report resource completion with the same event and microtask behavior for every script. */
pub(in crate::v9) fn completed(vm: &mut Vm, element: usize, kind: &str) -> EngineResult<()> {
    if vm.page.exists(element) {
        let fired = fire(vm, Target::Node(element), kind, false)
            .and_then(|_| vm.drain_jobs().map(|()| JsValue::Undefined));
        quiet(vm, fired).map_err(fatal)?;
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
    super::frames::adopt(page, old, new);
    if let Some(elements) = &mut page.state.parsed_scripts {
        for element in elements.iter_mut().filter(|element| **element == old) {
            *element = new;
        }
    }
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
