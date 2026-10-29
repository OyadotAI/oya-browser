/**
 * DevTools evaluation over a page's committed state, as V8's inspector does it
 * (v8-runtime-agent-impl.cc, injected-script.cc, value-mirror.cc) but over V9's own VM:
 * Runtime.evaluate and callFunctionOn compile the client's source as dynamic code and run it
 * against the live realm; results come back as RemoteObject mirrors, held by numeric handles
 * the page cannot see; getProperties, releaseObject and promise awaiting work on those handles.
 * Nothing here grants authority: the app decides whether the resulting Execution is committed.
 */
use super::{Page, dom, interface_tag, nodes};
use crate::protocol::{EngineError, EngineResult, SourceLocation};
use crate::v9::Compiled;
use crate::v9::builtins::{self, dynamic_base};
use crate::v9::value::{JsString, JsValue, ObjectId, number_to_string};
use crate::v9::vm::data::{Class, FunctionKind, HostData, PromiseState};
use crate::v9::vm::heap::{Key, Slot};
use crate::v9::vm::{GLOBAL, Limits, Res, Thrown, Vm};
use crate::v9::{Execution, PageInput, VmState, finish, on_v9_stack, session};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Properties shown in an object preview, as V8's `buildObjectPreview` name limit.
const PREVIEW_PROPERTIES: usize = 5;
/// Indexed elements shown in an array preview, as V8's index limit.
const PREVIEW_INDEXES: usize = 100;
/// Characters kept of a preview or description string before it is abbreviated.
const ABBREVIATE_CHARS: usize = 100;
/// Nesting `returnByValue` follows before refusing, as V8 refuses long reference chains.
const VALUE_DEPTH: usize = 32;
/// Values one `returnByValue` result may hold, so a cyclic or huge graph cannot fill a frame.
const VALUE_NODES: usize = 65_536;
/// Properties `getProperties` lists for one object.
const PROPERTY_LIMIT: usize = 10_000;
/// Hidden global key holding the handle table; the private mark keeps it out of reflection.
const HANDLES_KEY: &str = "devtools-handles";
/// Hidden global key holding each handle's object group name.
const GROUPS_KEY: &str = "devtools-groups";

/** Mirror options: V8's WrapOptions. */
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    /// Serialize the result as JSON instead of binding a handle.
    pub return_by_value: bool,
    /// Attach a shallow preview to object mirrors.
    pub preview: bool,
    /// Wait for a promise result; a pending promise comes back as `pending` with its handle.
    pub await_promise: bool,
    /// Object group released together by `ReleaseGroup`.
    pub group: String,
}

/** One argument of callFunctionOn: Runtime.CallArgument. */
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CallArgument {
    /// A JSON value.
    pub value: Option<Value>,
    /// `NaN`, `Infinity`, `-Infinity`, `-0` or a BigInt literal such as `12n`.
    pub unserializable: Option<String>,
    /// A handle bound by an earlier mirror.
    pub handle: Option<u32>,
}

/** What the front door asks of the realm. */
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// Runtime.evaluate: the page source at this index runs as a script; its completion value
    /// is the result. The app appends the client's text to the page's sources, so the code it
    /// compiles keeps its site across requests and never counts against the page's own eval.
    Evaluate {
        source: usize,
        #[serde(default)]
        options: Options,
    },
    /// Runtime.callFunctionOn: the page source at this index evaluates to a function, called
    /// with `this` and the arguments.
    CallFunction {
        source: usize,
        #[serde(default)]
        this: Option<u32>,
        #[serde(default)]
        arguments: Vec<CallArgument>,
        #[serde(default)]
        options: Options,
    },
    /// Runtime.getProperties of a handle.
    GetProperties {
        handle: u32,
        #[serde(default)]
        own_only: bool,
        #[serde(default)]
        accessors_only: bool,
        #[serde(default)]
        options: Options,
    },
    /// Runtime.awaitPromise: the state of a promise handle after microtasks drain.
    Await {
        handle: u32,
        #[serde(default)]
        options: Options,
    },
    /// Runtime.releaseObject for each handle.
    Release { handles: Vec<u32> },
    /// Runtime.releaseObjectGroup.
    ReleaseGroup { group: String },
    /// DOM.resolveNode: the wrapper of a node as a mirror.
    ResolveNode {
        node: usize,
        #[serde(default)]
        options: Options,
    },
    /// DOM.requestNode and describeNode(objectId): the node a handle wraps.
    NodeOf { handle: u32 },
    /// DOM.querySelector and querySelectorAll under a node, through V9's selector engine.
    QuerySelector {
        node: usize,
        selector: String,
        #[serde(default)]
        all: bool,
    },
}

/** The request's answer beside the Execution the app commits like any other step. */
#[derive(Deserialize, Serialize)]
pub struct Outcome {
    /// Changes, state and effects, exactly as a lifecycle step reports them.
    pub execution: Execution,
    /// The protocol-shaped result: `result`, `exceptionDetails`, `pending`, `handle`, `node`.
    pub result: Value,
    /// Console lines this request produced, for consoleAPICalled and exceptionThrown events.
    pub console: Vec<super::ConsoleEntry>,
    /// Whether the page must keep the source this request ran: a live function was compiled
    /// from it, or it declared a global. A one-off expression leaves no trace.
    pub retain: bool,
}

/** Run one request against the page's realm; `state` is None before V9 loaded the page. */
pub fn run(
    input: PageInput<'_>,
    state: Option<VmState>,
    request: Request,
) -> EngineResult<Outcome> {
    on_v9_stack(|| {
        let starts = crate::v9::offsets(input.sources);
        let lengths: Vec<usize> = input.sources.iter().map(String::len).collect();
        let (mut vm, compiled) = session(
            Limits::default(),
            input.sources,
            input.source_keys,
            true,
            Some(input.document),
            input.storage,
            input.host,
            input.environment,
            state,
            input.modules,
        )?;
        let Compiled::Each(codes) = compiled else {
            return Err(EngineError::invalid(
                "V9 compiled scripts in the wrong mode.",
            ));
        };
        let realm = Realm {
            codes,
            starts,
            lengths,
        };
        vm.remaining = vm.limits.operations;
        vm.page.throw_location = None;
        // The jar's current script-visible cookies, not the ones the page loaded with.
        vm.page.state.cookie = vm.page.environment.cookie.clone();
        let globals_before: Vec<String> = vm.globals.keys().cloned().collect();
        let ran = request.source();
        let result = dispatch(&mut vm, &realm, request)?;
        vm.frames.clear();
        vm.pins.clear();
        vm.collect_now().map_err(fatal)?;
        let retain = ran.is_some_and(|index| {
            let (start, end) = realm.range(index);
            vm.globals.keys().any(|name| !globals_before.contains(name))
                || references(&vm, start, end)
        });
        prune_dynamic(&mut vm);
        let console = vm.page.effects.console.clone();
        let execution = finish(vm)?;
        Ok(Outcome {
            execution,
            result,
            console,
            retain,
        })
    })
}

/** The page's compiled sources and where each starts, for requests that run one of them. */
struct Realm {
    codes: Vec<EngineResult<std::sync::Arc<crate::v9::compile::Code>>>,
    starts: Vec<usize>,
    lengths: Vec<usize>,
}

impl Realm {
    /** The compiled code of a source, or the syntax error it failed with. */
    fn code(&self, index: usize) -> EngineResult<std::sync::Arc<crate::v9::compile::Code>> {
        match self.codes.get(index) {
            Some(Ok(code)) => Ok(code.clone()),
            Some(Err(error)) => Err(error.clone()),
            None => Err(EngineError::invalid("No page source at that index.")),
        }
    }
    /** The page offsets a source spans. */
    fn range(&self, index: usize) -> (usize, usize) {
        let start = self.starts.get(index).copied().unwrap_or(0);
        (start, start + self.lengths.get(index).copied().unwrap_or(0))
    }
}

impl Request {
    /** The page source a request runs, if it runs one. */
    fn source(&self) -> Option<usize> {
        match self {
            Self::Evaluate { source, .. } | Self::CallFunction { source, .. } => Some(*source),
            _ => None,
        }
    }
}

/** Whether any live function was compiled from the byte offsets `start..=end`. */
fn references(vm: &Vm, start: usize, end: usize) -> bool {
    vm.heap.objects.values().any(|object| {
        matches!(
            object.function().map(|function| &function.kind),
            Some(FunctionKind::Closure { site, .. }) if (start..=end).contains(&site_offset(*site))
        )
    })
}

/** The page byte offset a code site names (sites carry a tag beside the offset). */
fn site_offset(site: u32) -> usize {
    (site / crate::v9::syntax::SITE_TAGS) as usize
}

/**
 * Drop the page's trailing dynamic sources (indirect eval, new Function) that no live function
 * was compiled from: a client evaluating through `eval` would otherwise fill the bounded list
 * one expression at a time. Earlier sources keep their offsets, so only the tail goes.
 */
fn prune_dynamic(vm: &mut Vm) {
    while let Some(last) = vm.dynamic.last() {
        let start = dynamic_base(&vm.dynamic[..vm.dynamic.len() - 1]);
        if references(vm, start, start + last.source.len()) {
            break;
        }
        vm.dynamic.pop();
    }
}

/** One request kind to its handler. */
fn dispatch(vm: &mut Vm, realm: &Realm, request: Request) -> EngineResult<Value> {
    match request {
        Request::Evaluate { source, options } => evaluate(vm, realm, source, &options),
        Request::CallFunction {
            source,
            this,
            arguments,
            options,
        } => call_function(vm, realm, source, this, &arguments, &options),
        Request::GetProperties {
            handle,
            own_only,
            accessors_only,
            options,
        } => get_properties(vm, handle, own_only, accessors_only, &options),
        Request::Await { handle, options } => await_handle(vm, handle, &options),
        Request::Release { handles } => {
            for handle in handles {
                release(vm, handle)?;
            }
            Ok(json!({}))
        }
        Request::ReleaseGroup { group } => release_group(vm, &group),
        Request::ResolveNode { node, options } => resolve_node(vm, node, &options),
        Request::NodeOf { handle } => node_of_handle(vm, handle),
        Request::QuerySelector {
            node,
            selector,
            all,
        } => query_selector(vm, node, &selector, all),
    }
}

/** The nodes under `node` (the document for its id) that match `selector`, in tree order. */
fn query_selector(vm: &mut Vm, node: usize, selector: &str, all: bool) -> EngineResult<Value> {
    let list = match super::selector::parse(selector) {
        Ok(list) => list,
        Err(message) => {
            return Ok(
                json!({"error":format!("Failed to execute 'querySelector' on 'Document': {message}")}),
            );
        }
    };
    let root = if node == dom::DOCUMENT {
        None
    } else {
        Some(node)
    };
    let candidates = match root {
        None => {
            let mut all_nodes = vec![nodes::HTML, nodes::HEAD];
            all_nodes.extend(vm.page.descendants(nodes::HEAD));
            all_nodes.push(nodes::BODY);
            all_nodes.extend(vm.page.descendants(nodes::BODY));
            all_nodes
        }
        Some(root) => vm.page.descendants(root),
    };
    let mut found = Vec::new();
    for candidate in candidates {
        if super::selector::matches(&mut vm.page, &vm.heap, candidate, &list, root) {
            found.push(candidate);
            if !all {
                break;
            }
        }
    }
    Ok(json!({"nodes":found}))
}

/** Run the page source at `source` at the top level; the completion value is the result. */
fn evaluate(vm: &mut Vm, realm: &Realm, source: usize, options: &Options) -> EngineResult<Value> {
    let (base, _) = realm.range(source);
    let code = match realm.code(source) {
        Ok(code) => code,
        Err(error) => return syntax_failure(vm, error, base, options),
    };
    vm.page.throw_location = None;
    let result = vm
        .run_script(code)
        .and_then(|value| vm.drain_jobs().map(|()| value));
    settle(vm, result, base, options)
}

/** The page source at `source` evaluates to a function; call it with `this` and the arguments. */
fn call_function(
    vm: &mut Vm,
    realm: &Realm,
    source: usize,
    this: Option<u32>,
    arguments: &[CallArgument],
    options: &Options,
) -> EngineResult<Value> {
    let receiver = match this {
        Some(handle) => lookup(vm, handle)?,
        None => JsValue::Undefined,
    };
    let mut args = Vec::with_capacity(arguments.len());
    for argument in arguments {
        args.push(resolve_argument(vm, argument)?);
    }
    let (base, _) = realm.range(source);
    let code = match realm.code(source) {
        Ok(code) => code,
        Err(error) => return syntax_failure(vm, error, base, options),
    };
    vm.page.throw_location = None;
    let function = match vm.run_script(code) {
        Ok(function) => function,
        Err(thrown) => return settle(vm, Err(thrown), base, options),
    };
    if !vm.is_callable(&function) {
        return Err(EngineError::invalid(
            "The functionDeclaration did not evaluate to a function.",
        ));
    }
    let result = vm
        .call(function, receiver, args)
        .and_then(|value| vm.drain_jobs().map(|()| value));
    settle(vm, result, base, options)
}

/** A source that did not compile reports a SyntaxError the way an uncaught one would. */
fn syntax_failure(
    vm: &mut Vm,
    error: EngineError,
    base: usize,
    options: &Options,
) -> EngineResult<Value> {
    if error.code != "script_syntax_error" && error.code != "unsupported_feature" {
        return Err(error);
    }
    let exception = vm
        .make_error(crate::v9::vm::error::ErrorKind::Syntax, &error.message)
        .map(JsValue::Object)?;
    vm.page.throw_location = error.location.map(|location| *location);
    thrown(vm, exception, base, options)
}

/** Package a completion: a mirror, exception details, or a pending promise handle. */
fn settle(
    vm: &mut Vm,
    result: Res<JsValue>,
    base: usize,
    options: &Options,
) -> EngineResult<Value> {
    let value = match result {
        Ok(value) => value,
        Err(Thrown::Value(exception)) => return thrown(vm, exception, base, options),
        Err(Thrown::Fatal(error)) => return Err(error),
    };
    if options.await_promise
        && let Some(id) = value.object()
        && let Some(Class::Promise(promise)) = vm.heap.get(id).map(|object| &object.class)
    {
        return match promise.state.clone() {
            PromiseState::Fulfilled(value) => wrap_result(vm, value, options),
            PromiseState::Rejected(reason) => rejected(vm, reason, options),
            PromiseState::Pending => {
                let handle = bind(vm, value, &options.group)?;
                Ok(json!({"pending":true,"handle":handle}))
            }
        };
    }
    wrap_result(vm, value, options)
}

/** `{result}` for a value. */
fn wrap_result(vm: &mut Vm, value: JsValue, options: &Options) -> EngineResult<Value> {
    let mirror = mirror(vm, &value, options)?;
    Ok(json!({"result":mirror}))
}

/** Runtime.awaitPromise: a settled promise's value, or still pending. */
fn await_handle(vm: &mut Vm, handle: u32, options: &Options) -> EngineResult<Value> {
    let value = lookup(vm, handle)?;
    let state = value
        .object()
        .and_then(|id| match vm.heap.get(id).map(|object| &object.class) {
            Some(Class::Promise(promise)) => Some(promise.state.clone()),
            _ => None,
        })
        .ok_or_else(|| EngineError::invalid("Could not find promise with given id"))?;
    vm.drain_jobs().map_err(fatal)?;
    match state {
        PromiseState::Fulfilled(value) => wrap_result(vm, value, options),
        PromiseState::Rejected(reason) => rejected(vm, reason, options),
        PromiseState::Pending => Ok(json!({"pending":true,"handle":handle})),
    }
}

/** A rejected promise reports its reason as an exception, as V8 does for awaited promises. */
fn rejected(vm: &mut Vm, reason: JsValue, options: &Options) -> EngineResult<Value> {
    let details = exception_details(vm, &reason, None, "Uncaught (in promise)", options)?;
    let result = mirror(vm, &reason, options)?;
    Ok(json!({"result":result,"exceptionDetails":details}))
}

/** An uncaught throw: the exception mirrored beside its location in the client's source. */
fn thrown(vm: &mut Vm, exception: JsValue, base: usize, options: &Options) -> EngineResult<Value> {
    let location = vm
        .page
        .throw_location
        .take()
        .filter(|location| location.byte_offset >= base);
    let details = exception_details(vm, &exception, location, "Uncaught", options)?;
    let result = mirror(vm, &exception, options)?;
    Ok(json!({"result":result,"exceptionDetails":details}))
}

/** Runtime.ExceptionDetails, with the frames V9 recorded at the throw as its stack trace. */
fn exception_details(
    vm: &mut Vm,
    exception: &JsValue,
    location: Option<SourceLocation>,
    text: &str,
    options: &Options,
) -> EngineResult<Value> {
    let trace = std::mem::take(&mut vm.page.throw_trace);
    let frames: Vec<Value> = trace
        .iter()
        .map(|frame| {
            let name = frame.split(" (").next().unwrap_or_default();
            json!({"functionName":name,"scriptId":"","url":"","lineNumber":0,"columnNumber":0})
        })
        .collect();
    let mirrored = mirror(
        vm,
        exception,
        &Options {
            return_by_value: false,
            ..options.clone()
        },
    )?;
    let mut details = json!({
        "exceptionId": next_exception_id(vm),
        "text": text,
        "lineNumber": location.as_ref().map_or(0, |at| at.line.saturating_sub(1)),
        "columnNumber": location.as_ref().map_or(0, |at| at.column.saturating_sub(1)),
        "exception": mirrored,
    });
    if !frames.is_empty() {
        details["stackTrace"] = json!({"callFrames":frames});
    }
    Ok(details)
}

/** Exception ids count up per page; the handle table's length is a free monotonic counter. */
fn next_exception_id(vm: &mut Vm) -> u32 {
    let table = handles(vm).unwrap_or(GLOBAL);
    let length = match vm.heap.get(table).map(|object| &object.class) {
        Some(Class::Array(elements)) => elements.len(),
        _ => 0,
    };
    length as u32 + 1
}

/** A Thrown that reaches the app as an engine error. */
fn fatal(thrown: Thrown) -> EngineError {
    match thrown {
        Thrown::Fatal(error) => error,
        Thrown::Value(_) => EngineError::invalid("V9 stopped on an uncaught internal exception."),
    }
}

/** Runtime.CallArgument to a value. */
fn resolve_argument(vm: &mut Vm, argument: &CallArgument) -> EngineResult<JsValue> {
    if let Some(handle) = argument.handle {
        return lookup(vm, handle);
    }
    if let Some(text) = &argument.unserializable {
        return match text.as_str() {
            "NaN" => Ok(JsValue::Number(f64::NAN)),
            "Infinity" => Ok(JsValue::Number(f64::INFINITY)),
            "-Infinity" => Ok(JsValue::Number(f64::NEG_INFINITY)),
            "-0" => Ok(JsValue::Number(-0.0)),
            other => other
                .strip_suffix('n')
                .and_then(|digits| crate::v9::value::BigInteger::parse(digits).ok())
                .map(JsValue::bigint)
                .ok_or_else(|| {
                    EngineError::invalid("Couldn't parse value object in call argument")
                }),
        };
    }
    match &argument.value {
        Some(value) => from_json(vm, value, 0).map_err(fatal),
        None => Ok(JsValue::Undefined),
    }
}

/** A JSON value as a page value, as `JSON.parse` would build it. */
fn from_json(vm: &mut Vm, value: &Value, depth: usize) -> Res<JsValue> {
    if depth > VALUE_DEPTH {
        return Err(EngineError::limit("call argument depth", VALUE_DEPTH).into());
    }
    Ok(match value {
        Value::Null => JsValue::Null,
        Value::Bool(flag) => JsValue::Boolean(*flag),
        Value::Number(number) => JsValue::Number(number.as_f64().unwrap_or(f64::NAN)),
        Value::String(text) => JsValue::String(JsString::from_utf8(text)?),
        Value::Array(items) => {
            let mut elements = Vec::with_capacity(items.len());
            for item in items {
                elements.push(from_json(vm, item, depth + 1)?);
            }
            JsValue::Object(vm.new_array(elements)?)
        }
        Value::Object(fields) => {
            let id = vm.new_object()?;
            for (name, field) in fields {
                let converted = from_json(vm, field, depth + 1)?;
                vm.put_key(id, Key::Str(JsString::from_utf8(name)?), converted)?;
            }
            JsValue::Object(id)
        }
    })
}

/** Runtime.getProperties: own (or inherited) properties as PropertyDescriptors. */
fn get_properties(
    vm: &mut Vm,
    handle: u32,
    own_only: bool,
    accessors_only: bool,
    options: &Options,
) -> EngineResult<Value> {
    let value = lookup(vm, handle)?;
    let Some(id) = value.object() else {
        return Err(EngineError::invalid("Value with given id is not an object"));
    };
    let mut result = Vec::new();
    let mut current = Some(id);
    let mut owner = true;
    while let Some(object) = current {
        let keys = vm.own_keys(object).map_err(fatal)?;
        for key in keys.into_iter().take(PROPERTY_LIMIT) {
            if let Some(entry) =
                describe_property(vm, object, &key, owner, accessors_only, options)?
            {
                result.push(entry);
            }
        }
        if own_only {
            break;
        }
        current = vm.proto_of(object);
        owner = false;
    }
    Ok(json!({"result":result}))
}

/** One Runtime.PropertyDescriptor; accessors are reported, never run. */
fn describe_property(
    vm: &mut Vm,
    object: ObjectId,
    key: &Key,
    is_own: bool,
    accessors_only: bool,
    options: &Options,
) -> EngineResult<Option<Value>> {
    let Some(prop) = vm.own(object, key).map_err(fatal)? else {
        return Ok(None);
    };
    let name = match key {
        Key::Str(text) => text.lossy(),
        Key::Sym(id) => format!("Symbol({})", builtins::symbol_text(vm, *id)),
        Key::Private(_) => return Ok(None),
    };
    let mut entry = json!({
        "name": name,
        "configurable": prop.attrs & crate::v9::vm::heap::CONFIGURABLE != 0,
        "enumerable": prop.attrs & crate::v9::vm::heap::ENUMERABLE != 0,
        "isOwn": is_own,
    });
    match prop.slot {
        Slot::Data(value) => {
            if accessors_only {
                return Ok(None);
            }
            entry["value"] = mirror(vm, &value, options)?;
            entry["writable"] = json!(prop.attrs & crate::v9::vm::heap::WRITABLE != 0);
        }
        Slot::Accessor(getter, setter) => {
            let getter = getter.map_or(JsValue::Undefined, JsValue::Object);
            let setter = setter.map_or(JsValue::Undefined, JsValue::Object);
            entry["get"] = mirror(vm, &getter, options)?;
            entry["set"] = mirror(vm, &setter, options)?;
        }
    }
    if let Key::Sym(_) = key {
        entry["symbol"] = mirror(vm, &key.value(), options)?;
    }
    Ok(Some(entry))
}

/** DOM.resolveNode: the node's wrapper, created if the page has not touched it yet. */
fn resolve_node(vm: &mut Vm, node: usize, options: &Options) -> EngineResult<Value> {
    let wrapper = dom::wrap(vm, node).map_err(fatal)?;
    if wrapper == JsValue::Null {
        return Err(EngineError::invalid("No node with given id found"));
    }
    wrap_result(vm, wrapper, options)
}

/** DOM.requestNode: which node a handle wraps. */
fn node_of_handle(vm: &mut Vm, handle: u32) -> EngineResult<Value> {
    let value = lookup(vm, handle)?;
    let node = dom::node_of(vm, &value)
        .ok_or_else(|| EngineError::invalid("Object id doesn't reference a Node"))?;
    Ok(json!({"node":node}))
}

/** The hidden handle table on the global object, created on first use. */
fn handles(vm: &mut Vm) -> EngineResult<ObjectId> {
    hidden_array(vm, HANDLES_KEY)
}

/** The hidden object-group table, parallel to the handle table. */
fn groups(vm: &mut Vm) -> EngineResult<ObjectId> {
    hidden_array(vm, GROUPS_KEY)
}

/** An array under a private-marked global key: rooted by the global, invisible to scripts. */
fn hidden_array(vm: &mut Vm, name: &str) -> EngineResult<ObjectId> {
    let key = hidden_key(name)?;
    if let Some(prop) = vm.own(GLOBAL, &key).map_err(fatal)?
        && let Slot::Data(JsValue::Object(id)) = prop.slot
        && vm.heap.get(id).is_some()
    {
        return Ok(id);
    }
    let array = vm.new_array(Vec::new()).map_err(fatal)?;
    vm.put_key(GLOBAL, key, JsValue::Object(array))
        .map_err(fatal)?;
    Ok(array)
}

/** A key no script can spell: an engine-private key. */
fn hidden_key(name: &str) -> EngineResult<Key> {
    Ok(Key::private(name))
}

/** Bind a value to a new handle (ids count up from 1, as V8's `bindObject`). */
fn bind(vm: &mut Vm, value: JsValue, group: &str) -> EngineResult<u32> {
    let table = handles(vm)?;
    let groups = groups(vm)?;
    vm.array_append(&JsValue::Object(table), value)
        .map_err(fatal)?;
    let name = JsValue::String(JsString::from_utf8(group)?);
    vm.array_append(&JsValue::Object(groups), name)
        .map_err(fatal)?;
    let length = match vm.heap.get(table).map(|object| &object.class) {
        Some(Class::Array(elements)) => elements.len(),
        _ => 0,
    };
    Ok(length as u32)
}

/** The value a handle names; a released or unknown handle is the error V8 gives. */
fn lookup(vm: &mut Vm, handle: u32) -> EngineResult<JsValue> {
    let table = handles(vm)?;
    let index = (handle as usize).checked_sub(1);
    let found = index.and_then(
        |index| match vm.heap.get(table).map(|object| &object.class) {
            Some(Class::Array(elements)) => elements.get(index).cloned(),
            _ => None,
        },
    );
    match found {
        Some(JsValue::Hole) | None => {
            Err(EngineError::invalid("Could not find object with given id"))
        }
        Some(value) => Ok(value),
    }
}

/** Runtime.releaseObject: the slot becomes a hole and the value may be collected. */
fn release(vm: &mut Vm, handle: u32) -> EngineResult<()> {
    let table = handles(vm)?;
    if let Some(index) = (handle as usize).checked_sub(1)
        && let Some(Class::Array(elements)) = vm.heap.get_mut(table).map(|object| &mut object.class)
        && index < elements.len()
    {
        elements.set(index, JsValue::Hole);
    }
    Ok(())
}

/** Runtime.releaseObjectGroup: every handle bound under the group name. */
fn release_group(vm: &mut Vm, group: &str) -> EngineResult<Value> {
    if group.is_empty() {
        return Ok(json!({}));
    }
    let groups = groups(vm)?;
    let named: Vec<usize> = match vm.heap.get(groups).map(|object| &object.class) {
        Some(Class::Array(names)) => names
            .iter()
            .enumerate()
            .filter(|(_, name)| matches!(name, JsValue::String(text) if text.is(group)))
            .map(|(index, _)| index)
            .collect(),
        _ => Vec::new(),
    };
    for index in named {
        release(vm, index as u32 + 1)?;
    }
    Ok(json!({}))
}

/** A Runtime.RemoteObject for a value: V8's value mirror over V9's classes. */
fn mirror(vm: &mut Vm, value: &JsValue, options: &Options) -> EngineResult<Value> {
    Ok(match value {
        JsValue::Undefined | JsValue::Hole => json!({"type":"undefined"}),
        JsValue::Null => json!({"type":"object","subtype":"null","value":null}),
        JsValue::Boolean(flag) => json!({"type":"boolean","value":flag}),
        JsValue::String(text) => json!({"type":"string","value":text.lossy()}),
        #[cfg(feature = "conformance")]
        JsValue::Smi(number) => number_mirror(*number),
        JsValue::Number(number) => number_mirror(*number),
        #[cfg(feature = "conformance")]
        JsValue::SharedNumber(number) | JsValue::LocalNumber(number) => number_mirror(number.value),
        JsValue::BigInt(big) => {
            json!({"type":"bigint","unserializableValue":format!("{big}n"),"description":format!("{big}n")})
        }
        JsValue::BigIntWide(big) => {
            vm.charge(big.format_work(10)).map_err(fatal)?;
            json!({"type":"bigint","unserializableValue":format!("{big}n"),"description":format!("{big}n")})
        }
        JsValue::Symbol(id) => {
            let description = format!("Symbol({})", builtins::symbol_text(vm, *id));
            let mut object = json!({"type":"symbol","description":description});
            if !options.return_by_value {
                object["objectId"] = json!(bind(vm, value.clone(), &options.group)?.to_string());
            }
            object
        }
        JsValue::Object(id) => object_mirror(vm, *id, options)?,
    })
}

/** Numbers: finite ones by value, the rest by their unserializable spelling, as V8. */
fn number_mirror(number: f64) -> Value {
    let description = number_to_string(number);
    if number.is_nan() {
        return json!({"type":"number","unserializableValue":"NaN","description":"NaN"});
    }
    if number.is_infinite() {
        return json!({"type":"number","unserializableValue":description,"description":description});
    }
    if number == 0.0 && number.is_sign_negative() {
        return json!({"type":"number","unserializableValue":"-0","description":"0"});
    }
    json!({"type":"number","value":json_number(number),"description":description})
}

/** A finite number as JSON: integral values without a fraction, as V8 serializes them. */
fn json_number(number: f64) -> Value {
    if number.fract() == 0.0 && number.abs() < i64::MAX as f64 {
        return Value::from(number as i64);
    }
    serde_json::Number::from_f64(number).map_or(Value::Null, Value::Number)
}

/** An object's mirror: type, subtype, className, description, then a handle or a value. */
fn object_mirror(vm: &mut Vm, id: ObjectId, options: &Options) -> EngineResult<Value> {
    let (kind, subtype, class_name, description) = classify(vm, id);
    let mut object = json!({"type":kind,"className":class_name,"description":description});
    if let Some(subtype) = subtype {
        object["subtype"] = json!(subtype);
    }
    if let Some(node) = dom::node_of(vm, &JsValue::Object(id)) {
        object["marsNode"] = json!(node);
    }
    if options.return_by_value {
        let mut budget = VALUE_NODES;
        object["value"] = to_json(vm, &JsValue::Object(id), 0, &mut budget).map_err(fatal)?;
        return Ok(object);
    }
    if options.preview {
        object["preview"] = preview(vm, id, subtype, &description)?;
    }
    object["objectId"] = json!(bind(vm, JsValue::Object(id), &options.group)?.to_string());
    Ok(object)
}

/** V8's `ValueMirror::create` subtype table over V9's object classes. */
fn classify(vm: &mut Vm, id: ObjectId) -> (&'static str, Option<&'static str>, String, String) {
    // The document wrapper is an intrinsic, not a heap object; nodes come first.
    if let Some(node) = dom::node_of(vm, &JsValue::Object(id)) {
        let interface = interface_tag(vm, id).unwrap_or_else(|| "HTMLElement".into());
        return (
            "object",
            Some("node"),
            interface,
            node_description(&mut vm.page, node),
        );
    }
    let class = vm.heap.get(id).map(|object| object.class.clone());
    let name = constructor_name(vm, id);
    match class {
        Some(Class::Function(_)) | None if vm.is_callable(&JsValue::Object(id)) => {
            let text = vm
                .to_text(&JsValue::Object(id))
                .unwrap_or_else(|_| "function () {}".into());
            ("function", None, "Function".into(), text)
        }
        Some(Class::Array(elements)) => (
            "object",
            Some("array"),
            name.clone(),
            format!("{name}({})", elements.len()),
        ),
        Some(Class::Error) => ("object", Some("error"), name, error_description(vm, id)),
        Some(Class::Date(time)) => {
            let text = vm
                .to_text(&JsValue::Object(id))
                .unwrap_or_else(|_| number_to_string(time));
            ("object", Some("date"), "Date".into(), text)
        }
        Some(Class::RegExp(data)) => {
            let text = format!("/{}/{}", data.source.lossy(), data.flags);
            ("object", Some("regexp"), "RegExp".into(), text)
        }
        Some(Class::Map(table)) => (
            "object",
            Some("map"),
            "Map".into(),
            format!("Map({})", table.live),
        ),
        Some(Class::Set(table)) => (
            "object",
            Some("set"),
            "Set".into(),
            format!("Set({})", table.live),
        ),
        Some(Class::WeakMap(_)) => (
            "object",
            Some("weakmap"),
            "WeakMap".into(),
            "WeakMap".into(),
        ),
        Some(Class::WeakSet(_)) => (
            "object",
            Some("weakset"),
            "WeakSet".into(),
            "WeakSet".into(),
        ),
        Some(Class::Promise(_)) => (
            "object",
            Some("promise"),
            "Promise".into(),
            "Promise".into(),
        ),
        Some(Class::Proxy(_)) => ("object", Some("proxy"), "Proxy".into(), "Proxy".into()),
        Some(Class::Generator(_)) => ("object", Some("generator"), name.clone(), name),
        Some(Class::Iter(_)) => ("object", Some("iterator"), name.clone(), name),
        Some(Class::ArrayBuffer(bytes)) => (
            "object",
            Some("arraybuffer"),
            "ArrayBuffer".into(),
            format!("ArrayBuffer({})", bytes.0.len()),
        ),
        Some(Class::View(view)) => {
            let subtype = if matches!(view.kind, crate::v9::vm::data::ElementKind::DataView) {
                "dataview"
            } else {
                "typedarray"
            };
            (
                "object",
                Some(subtype),
                name.clone(),
                format!("{name}({})", view.length),
            )
        }
        Some(Class::Host(data)) => host_mirror(vm, id, &data),
        Some(
            Class::Boolean(_)
            | Class::Number(_)
            | Class::String(_)
            | Class::Symbol(_)
            | Class::BigInt(_)
            | Class::BigIntWide(_),
        ) => ("object", None, name.clone(), name),
        _ if id == GLOBAL => ("object", None, "Window".into(), "Window".into()),
        _ => ("object", None, name.clone(), name),
    }
}

/** Nodes and the other host interfaces, named the way Chrome's DOM wrapper mirrors are. */
fn host_mirror(
    vm: &mut Vm,
    id: ObjectId,
    data: &HostData,
) -> (&'static str, Option<&'static str>, String, String) {
    let interface = interface_tag(vm, id).unwrap_or_else(|| "Object".into());
    match data {
        HostData::Node(node) | HostData::Document(node) => (
            "object",
            Some("node"),
            interface,
            node_description(&mut vm.page, *node),
        ),
        HostData::NodeList(nodes) => (
            "object",
            Some("array"),
            interface.clone(),
            format!("{interface}({})", nodes.len()),
        ),
        HostData::Live(list) => {
            let count = crate::v9::platform::collections::current(vm, list).len();
            (
                "object",
                Some("array"),
                interface.clone(),
                format!("{interface}({count})"),
            )
        }
        HostData::Children(node, _) => {
            let count = vm.page.children(*node).len();
            (
                "object",
                Some("array"),
                interface.clone(),
                format!("{interface}({count})"),
            )
        }
        _ => ("object", None, interface.clone(), interface),
    }
}

/** `div#id.class.other` for elements, `#text` for text nodes, as `descriptionForNode`. */
fn node_description(page: &mut Page, node: usize) -> String {
    if node == dom::DOCUMENT {
        return "#document".into();
    }
    let Some(view) = page.view(node) else {
        return "#node".into();
    };
    if view.tag.starts_with('#') {
        return view.tag;
    }
    let mut description = view.tag.clone();
    if let Some(id) = view.attributes.get("id").filter(|id| !id.is_empty()) {
        description.push('#');
        description.push_str(id);
    }
    if let Some(classes) = view.attributes.get("class") {
        for class in classes.split_whitespace() {
            description.push('.');
            description.push_str(class);
        }
    }
    description
}

/** `Name: message` plus the recorded stack, as `descriptionForError`. */
fn error_description(vm: &mut Vm, id: ObjectId) -> String {
    let value = JsValue::Object(id);
    let mut description = vm.describe(&value);
    if let Ok(Some(prop)) = vm.own(id, &Key::from("stack"))
        && let Slot::Data(JsValue::String(stack)) = prop.slot
    {
        let stack = stack.lossy();
        if let Some(at) = stack.find("\n    at ") {
            description.push_str(&stack[at..]);
        }
    }
    description
}

/** The constructor's name along the prototype chain, read as data without running getters. */
fn constructor_name(vm: &mut Vm, id: ObjectId) -> String {
    let mut current = Some(id);
    for _ in 0..VALUE_DEPTH {
        let Some(object) = current else {
            break;
        };
        if let Ok(Some(prop)) = vm.own(object, &Key::constructor())
            && let Slot::Data(JsValue::Object(constructor)) = prop.slot
            && let Ok(Some(name)) = vm.own(constructor, &Key::from("name"))
            && let Slot::Data(JsValue::String(text)) = name.slot
            && !text.is_empty()
        {
            return text.lossy();
        }
        current = vm.proto_of(object);
    }
    "Object".into()
}

/** Runtime.ObjectPreview: a few own data properties with abbreviated values. */
fn preview(
    vm: &mut Vm,
    id: ObjectId,
    subtype: Option<&str>,
    description: &str,
) -> EngineResult<Value> {
    let is_array = subtype == Some("array");
    let limit = if is_array {
        PREVIEW_INDEXES
    } else {
        PREVIEW_PROPERTIES
    };
    let keys = vm.enumerable_keys(id).map_err(fatal)?;
    let mut properties = Vec::new();
    for key in keys.iter().take(limit) {
        let Some(prop) = vm.own(id, key).map_err(fatal)? else {
            continue;
        };
        let name = crate::v9::vm::property::key_text(key);
        properties.push(match prop.slot {
            Slot::Data(value) => property_preview(vm, &name, &value),
            Slot::Accessor(..) => json!({"name":name,"type":"accessor"}),
        });
    }
    let mut result = json!({"type":"object","description":abbreviate(description),"overflow":keys.len() > limit,"properties":properties});
    if let Some(subtype) = subtype {
        result["subtype"] = json!(subtype);
    }
    Ok(result)
}

/** Runtime.PropertyPreview: the value's type and a short string form. */
fn property_preview(vm: &mut Vm, name: &str, value: &JsValue) -> Value {
    let (kind, subtype, text) = match value {
        JsValue::Undefined | JsValue::Hole => ("undefined", None, "undefined".into()),
        JsValue::Null => ("object", Some("null"), "null".into()),
        JsValue::Boolean(flag) => ("boolean", None, flag.to_string()),
        #[cfg(feature = "conformance")]
        JsValue::Smi(number) => ("number", None, number_to_string(*number)),
        JsValue::Number(number) => ("number", None, number_to_string(*number)),
        #[cfg(feature = "conformance")]
        JsValue::SharedNumber(number) | JsValue::LocalNumber(number) => {
            ("number", None, number_to_string(number.value))
        }
        JsValue::String(string) => ("string", None, string.lossy()),
        JsValue::BigInt(big) => ("bigint", None, format!("{big}n")),
        JsValue::BigIntWide(big) => ("bigint", None, format!("{}n", big.diagnostic())),
        JsValue::Symbol(id) => (
            "symbol",
            None,
            format!("Symbol({})", builtins::symbol_text(vm, *id)),
        ),
        JsValue::Object(id) => {
            let (kind, subtype, _, description) = classify(vm, *id);
            (kind, subtype, description)
        }
    };
    let mut preview = json!({"name":name,"type":kind,"value":abbreviate(&text)});
    if let Some(subtype) = subtype {
        preview["subtype"] = json!(subtype);
    }
    preview
}

/** Cut a long string in the middle with an ellipsis, as V8's `abbreviateString`. */
fn abbreviate(text: &str) -> String {
    let count = text.chars().count();
    if count <= ABBREVIATE_CHARS {
        return text.into();
    }
    let half = ABBREVIATE_CHARS / 2;
    let head: String = text.chars().take(half).collect();
    let tail: String = text.chars().skip(count - half).collect();
    format!("{head}…{tail}")
}

/**
 * `returnByValue`: arrays by index, other objects by enumerable own string keys, primitives
 * as themselves; functions, symbols and holes become undefined (dropped from objects, null in
 * arrays), nodes and other hosts become `{}`, as V8's protocol value conversion does.
 */
fn to_json(vm: &mut Vm, value: &JsValue, depth: usize, budget: &mut usize) -> Res<Value> {
    if depth > VALUE_DEPTH {
        return Err(vm.type_error("Object reference chain is too long"));
    }
    if *budget == 0 {
        return Err(vm.type_error("Object couldn't be returned by value"));
    }
    *budget -= 1;
    Ok(match value {
        JsValue::Undefined | JsValue::Hole | JsValue::Symbol(_) => Value::Null,
        JsValue::Null => Value::Null,
        JsValue::Boolean(flag) => Value::Bool(*flag),
        #[cfg(feature = "conformance")]
        JsValue::Smi(number) => json_number(*number),
        JsValue::Number(number) => json_number(*number),
        #[cfg(feature = "conformance")]
        JsValue::SharedNumber(number) | JsValue::LocalNumber(number) => json_number(number.value),
        JsValue::String(text) => Value::String(text.lossy()),
        JsValue::BigInt(_) | JsValue::BigIntWide(_) => {
            return Err(vm.type_error("BigInt value can't be returned by value"));
        }
        JsValue::Object(id) => object_to_json(vm, *id, depth, budget)?,
    })
}

/** The object part of `to_json`. */
fn object_to_json(vm: &mut Vm, id: ObjectId, depth: usize, budget: &mut usize) -> Res<Value> {
    if vm.is_callable(&JsValue::Object(id)) {
        return Ok(Value::Null);
    }
    let class = vm.heap.get(id).map(|object| object.class.clone());
    match class {
        Some(Class::Array(elements)) => {
            if elements.len() > *budget {
                return Err(vm.type_error("Array exceeds the result serialization budget"));
            }
            let mut items = Vec::with_capacity(elements.len());
            for element in elements {
                items.push(to_json(vm, &element, depth + 1, budget)?);
            }
            Ok(Value::Array(items))
        }
        Some(Class::Boolean(flag)) => Ok(Value::Bool(flag)),
        Some(Class::Number(number)) => Ok(json_number(number)),
        Some(Class::String(text)) => Ok(Value::String(text.lossy())),
        Some(Class::Date(_)) => {
            let method = vm.get(&JsValue::Object(id), &Key::from("toISOString"))?;
            let text = vm.call(method, JsValue::Object(id), Vec::new())?;
            Ok(Value::String(vm.to_text(&text)?))
        }
        Some(Class::Host(_)) => Ok(Value::Object(Map::new())),
        _ => {
            let mut fields = Map::new();
            for key in vm.enumerable_keys(id)? {
                let Some(prop) = vm.own(id, &key)? else {
                    continue;
                };
                let Slot::Data(value) = prop.slot else {
                    continue;
                };
                if matches!(
                    value,
                    JsValue::Undefined | JsValue::Hole | JsValue::Symbol(_)
                ) || vm.is_callable(&value)
                {
                    continue;
                }
                fields.insert(
                    crate::v9::vm::property::key_text(&key),
                    to_json(vm, &value, depth + 1, budget)?,
                );
            }
            Ok(Value::Object(fields))
        }
    }
}

/** The identities V9 gives the document and its virtual html, head and body, plus the first
script-created identity, so the app's DOM view names head children the way V9 numbers them. */
pub fn virtual_nodes() -> VirtualNodes {
    VirtualNodes {
        document: dom::DOCUMENT,
        html: nodes::HTML,
        head: nodes::HEAD,
        body: nodes::BODY,
        detached_base: super::nodes_detached_base(),
    }
}

/** Node identities outside the app's projection. */
#[derive(Clone, Copy, Debug)]
pub struct VirtualNodes {
    /// The document node.
    pub document: usize,
    /// The html element.
    pub html: usize,
    /// The head element.
    pub head: usize,
    /// The body element; projected roots are its children.
    pub body: usize,
    /// First identity of script-created and metadata head nodes, numbered in creation order.
    pub detached_base: usize,
}
