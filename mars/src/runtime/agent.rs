/** Native agents manipulate browser-owned Rust state through a discoverable protocol. */
#[path = "agent/input.rs"]
mod input;
#[path = "agent/keyboard.rs"]
mod keyboard;
#[path = "agent/lanes.rs"]
mod lanes;
#[path = "agent/raster.rs"]
mod raster;
pub(crate) use raster::base64;
#[path = "agent/traces.rs"]
mod traces;
use super::authority::{self, Owner, Permit};
use super::page::Empty;
use super::supervisor::Runtime;
use crate::constants::*;
use crate::protocol::{
    EngineError, EngineResult, Request, Response, decode, parameters, read_frame, valid_id,
    write_message,
};
use serde_json::{Value, json};
use std::io::{BufRead, Write};

/** Native operation dispatch is a table, independent of CDP or injected page source. */
type Handler = fn(&mut Runtime, Value) -> EngineResult<Value>;

/** Tab-scoped native operations never infer a target from current selection. */
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TabTarget {
    tab_id: String,
}

/** Control status can inspect one exact tab or the visible tab by default. */
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlTarget {
    #[serde(default)]
    tab_id: Option<String>,
}

impl Runtime {
    /// One owner executes one request at a time; caller IDs never imply replay.
    pub fn handle(&mut self, request: Request) -> EngineResult<Value> {
        let save_session = session_mutation(&request.op);
        let shutting_down = request.op == "runtime.shutdown";
        let result = self.handle_request(request);
        if result.is_ok() && !shutting_down && (save_session || self.session_dirty) {
            let timed = std::time::Instant::now();
            self.persist_session();
            super::supervisor::perf::record("agent.persist_session", timed, String::new);
        }
        result
    }

    /// Dispatch one request after the outer boundary records a committed session change.
    fn handle_request(&mut self, request: Request) -> EngineResult<Value> {
        request.validate()?;
        match request.op.as_str() {
            "tab.list" => {
                let _: Empty = parameters(request.params)?;
                return self.tabs_value();
            }
            "tab.open" => {
                let _: Empty = parameters(request.params)?;
                self.require_human_tab_action()?;
                return self.open_tab();
            }
            "tab.select" | "tab.close" => {
                let input: TabTarget = parameters(request.params)?;
                self.require_human_tab_action()?;
                return if request.op == "tab.select" {
                    self.select_tab_by_id(&input.tab_id)
                } else {
                    self.close_tab(&input.tab_id)
                };
            }
            _ => {}
        }
        if request.op.starts_with("page.") {
            let id = self.page_tab_id(&request.params)?;
            let mut active = request;
            remove_tab_id(&mut active.params);
            let result = self.in_tab(&id, |runtime| runtime.handle_active(active))?;
            return add_tab_id(result, &id);
        }
        self.handle_active(request)
    }

    /**
     * Run a page operation in one tab without changing the person's visible selection; the
     * background tab's page and history are swapped in and stored back exactly once.
     */
    fn in_tab<T>(
        &mut self,
        id: &str,
        operation: impl FnOnce(&mut Self) -> EngineResult<T>,
    ) -> EngineResult<T> {
        // Background work never moves the person's visible selection.
        self.run_in_tab(id, operation)
    }

    /// Existing page commands execute only after the tab wrapper selected their state.
    fn handle_active(&mut self, request: Request) -> EngineResult<Value> {
        let timed = std::time::Instant::now();
        self.record_revision();
        super::supervisor::perf::record("agent.record_revision", timed, String::new);
        let result = self.dispatch_active(request);
        let timed = std::time::Instant::now();
        self.record_revision();
        super::supervisor::perf::record("agent.record_revision", timed, String::new);
        result
    }

    /// Dispatch on the active tab without recording, so a batch's steps do not evict history.
    pub(super) fn dispatch_active(&mut self, request: Request) -> EngineResult<Value> {
        request.validate()?;
        if let Some(error) = self.profile_error
            && requires_profile(&request.op)
        {
            return Err(error.engine_error());
        }
        if mutating(&request.op) && self.authority.is_some() && self.permit.is_none() {
            return Err(authority::denied());
        }
        let handlers: &[(&str, Handler)] = &[
            ("runtime.capabilities", Self::capabilities),
            ("runtime.shutdown", Self::shutdown),
            ("page.navigate", Self::navigate),
            ("page.inspect", Self::inspect_with),
            ("page.summary", Self::summary),
            ("page.find", Self::find_nodes),
            ("page.diff", Self::diff_since),
            ("page.wait", Self::wait_condition),
            ("page.act", Self::act),
            ("page.set_text", Self::set_text),
            ("page.layout", Self::layout),
            ("page.click", Self::click),
            ("page.type", Self::type_text),
            ("page.back", Self::back),
            ("page.forward", Self::forward),
            ("page.reload", Self::reload),
            ("page.scroll", Self::scroll),
            ("page.set_checked", Self::check),
            ("page.select", Self::select_option),
            ("page.submit", Self::submit),
            ("page.screenshot", Self::screenshot),
            ("page.click_at", Self::click_at),
            ("page.hover", Self::hover),
            ("page.scroll_at", Self::scroll_at),
            ("page.press", Self::press),
            ("page.console", Self::console),
            ("page.network", Self::network),
            ("control.status", Self::control_status),
            ("control.release", Self::release_control),
            ("page.devtools", Self::devtools),
            ("page.devtools_peek", Self::devtools_peek),
            ("page.nodes", Self::nodes),
            ("page.history", Self::history_position),
            ("page.outer_html", Self::outer_html),
            ("page.set_attribute", Self::set_attribute),
            ("page.scroll_into_view", Self::scroll_into_view),
            ("page.cookies_list", Self::cookies_list),
            ("page.cookies_set", Self::cookies_set),
            ("page.cookies_delete", Self::cookies_delete),
            ("page.cookies_clear", Self::cookies_clear),
            ("page.emulate", Self::emulate),
            ("page.navigate_intercepted", Self::navigate_intercepted),
        ];
        let handler = handlers
            .iter()
            .find(|(name, _)| *name == request.op)
            .map(|(_, handler)| handler)
            .ok_or_else(|| {
                EngineError::new(
                    "unsupported_operation",
                    "Unknown native operation.",
                    "Call runtime.capabilities for supported operations.",
                )
            })?;
        if request.op.starts_with("page.") {
            self.healthy()?;
        }
        handler(self, request.params)
    }

    /// Desktop page operations require an exact discovered tab identity.
    fn page_tab_id(&self, params: &Value) -> EngineResult<String> {
        let id = params.get("tab_id").and_then(Value::as_str);
        match id {
            Some(id) => Ok(id.into()),
            None if self.web_origins.is_none() => Ok(self.selected_tab_id().into()),
            None => Err(EngineError::invalid("Page operations require tab_id.")),
        }
    }

    /// Tab creation, selection and closure are local human actions only.
    fn require_human_tab_action(&self) -> EngineResult<()> {
        if self
            .permit
            .is_some_and(|permit| permit.owner == Owner::Agent)
        {
            return Err(authority::denied());
        }
        Ok(())
    }

    /// Desktop admissions carry local authority rather than trusting page/agent parameters.
    pub fn handle_as(&mut self, mut request: Request, permit: Permit) -> EngineResult<Value> {
        request.validate()?;
        let lane = lanes::take_lane(&mut request.params)?;
        if permit.owner == Owner::Agent && ["tab.open", "tab.close"].contains(&request.op.as_str())
        {
            self.permit = Some(permit);
            let result = self.agent_tab(request, permit, &lane);
            self.permit = None;
            return result;
        }
        if mutating(&request.op)
            && let Some(authority) = &self.authority
        {
            let tab_id = if request.op.starts_with("page.") {
                request
                    .params
                    .get("tab_id")
                    .and_then(Value::as_str)
                    .map(super::supervisor::tab_key)
                    .transpose()?
            } else {
                permit.tab_id
            };
            let mut state = authority::lock(authority)?;
            let local_tab_action = request.op.starts_with("tab.")
                && permit.owner == Owner::Human
                && permit.tab_id.is_none();
            if local_tab_action {
                // The window dispatches these only from native human actions.
            } else if let Some(tab_id) = tab_id {
                state.validate_tab(permit, tab_id)?;
                if permit.owner == Owner::Agent {
                    state.claim(tab_id, &lane)?;
                }
            } else {
                state.validate(permit)?;
            }
            if permit.owner == Owner::Agent {
                let epoch = request
                    .params
                    .as_object_mut()
                    .and_then(|params| params.remove("control_epoch"))
                    .and_then(|value| value.as_u64());
                if epoch != Some(permit.epoch) {
                    return Err(authority::denied());
                }
            }
        }
        self.permit = Some(permit);
        let result = self.handle(request);
        self.permit = None;
        result
    }

    /** Inspection is permitted without granting mutation authority. */
    fn control_status(&mut self, params: Value) -> EngineResult<Value> {
        let input: ControlTarget = parameters(params)?;
        if let Some(authority) = &self.authority {
            let state = authority::lock(authority)?;
            let id = input
                .tab_id
                .as_deref()
                .map(super::supervisor::tab_key)
                .transpose()?
                .unwrap_or(super::supervisor::tab_key(self.selected_tab_id())?);
            let (owner, epoch) = state.tab_status(id).ok_or_else(authority::denied)?;
            return Ok(json!({"tab_id":format!("tab-{id}"),"owner":owner,"epoch":epoch}));
        }
        Ok(json!({"tab_id":self.selected_tab_id(),"owner":"agent","epoch":1}))
    }
    /** Agents can yield control but have no operation to grant it to themselves. */
    fn release_control(&mut self, params: Value) -> EngineResult<Value> {
        let input: ControlTarget = parameters(params)?;
        let authority = self.authority.as_ref().ok_or_else(|| {
            EngineError::unsupported("Headless agent sessions have no human ownership surface.")
        })?;
        let permit = self.permit.ok_or_else(authority::denied)?;
        let tab_id = permit.tab_id.ok_or_else(authority::denied)?;
        if input
            .tab_id
            .as_deref()
            .map(super::supervisor::tab_key)
            .transpose()?
            .is_some_and(|requested| requested != tab_id)
        {
            return Err(authority::denied());
        }
        let mut state = authority::lock(authority)?;
        state.validate_tab(permit, tab_id)?;
        state.change_tab(tab_id, Owner::Human)?;
        let (owner, epoch) = state.tab_status(tab_id).ok_or_else(authority::denied)?;
        Ok(json!({"tab_id":format!("tab-{tab_id}"),"owner":owner,"epoch":epoch}))
    }

    /** Capabilities remain readable after a fault and disclose unavailable production features. */
    fn capabilities(&mut self, params: Value) -> EngineResult<Value> {
        let _: Empty = parameters(params)?;
        let _ = self.healthy();
        let mut result = json!({"protocol_version":VERSION,"product_goal":"electron_replacement","implementation_status":"in_development","state":if self.failed {"failed"} else {"ready"},
            "operations":{"runtime.capabilities":{},"runtime.shutdown":{},"page.navigate":{"url":"string"},"page.inspect":{},"page.set_text":{"page_id":"string","node_id":"string","expected_revision":"u64","text":"string"},"page.layout":{}},
            "engines":self.process_ids().into_iter().map(|(name,pid)|json!({"name":name,"pid":pid})).collect::<Vec<_>>(),
            "subsets":{"network":"literal loopback HTTP GET; UTF-8 HTML; no redirects","document":"bounded explicit HTML tree, classic inline/external scripts run blocking, then defer, then async, each in document order, text, links, inputs and buttons","v9":"ordered classic scripts share page state and budgets; typed values; own-property objects and arrays; lexical bindings; named functions with parameters and returns; if/while; arithmetic and comparisons; input reads and textContent writes","web_storage":{"localStorage":{"scope":"canonical origin","methods":["getItem","setItem","removeItem","clear"]},"sessionStorage":{"scope":"tab and canonical origin","methods":["getItem","setItem","removeItem","clear"]}},"layout":LAYOUT_UNIT},
            "unavailable":["desktop_window","pixels","css","full_javascript","https","internet","frames","named_profiles","electron_profile_import","cookie_inspection","human_agent_handoff","os_sandbox"],
            "limits":{"frame_bytes":FRAME_BYTES,"html_bytes":HTML_BYTES,"header_bytes":HEADER_BYTES,"nodes":NODES,"text_bytes":TEXT_BYTES,"script_bytes":SCRIPT_BYTES,"script_total_bytes":SCRIPT_TOTAL_BYTES,"script_count":SCRIPT_COUNT,"statements":STATEMENTS,"tokens":TOKENS,"operations":OPERATIONS,"expression_nesting":EXPRESSION_DEPTH,"js_string_units":JS_STRING_UNITS,"js_environment_units":JS_ENVIRONMENT_UNITS,"vm_state_bytes":crate::constants::VM_STATE_BYTES,"vm_objects":crate::constants::VM_OBJECTS,"vm_properties":crate::constants::VM_PROPERTIES,"vm_array_length":crate::constants::VM_ARRAY_LENGTH,"function_parameters":crate::constants::FUNCTION_PARAMETERS,"url_bytes":URL_BYTES,"id_bytes":ID_BYTES,"network_deadline_ms":NETWORK_TIMEOUT.as_millis(),"rpc_deadline_ms":RPC_TIMEOUT.as_millis()}});
        result["subsets"]["v9"] = json!(
            "ordered classic scripts; automatic semicolon insertion in the supported classic-script grammar; numeric, UTF-16 string, Boolean, null and undefined values; own-property objects and arrays; UTF-16 string length and indexed reads; string trim and literal-separator split; array map, filter and join with bounded callable processing; array some and find with bounded callable predicates; array-literal spread of owned arrays and primitive strings, with bounded code-point string iteration; lexical let/const and reassignment; function-scoped and script-global var declarations; declaration hoisting and supported var/function redeclarations; first-class functions; function expressions and nested declarations; callbacks and lexical closures with shared captured bindings; arrow functions with simple parameters, expression/block bodies and lexical capture; receiver-aware ordinary function calls; this expressions; lexical this capture in arrows; supported strict and non-strict receiver behavior; Function.prototype.call, apply and bind; bound receivers and prefixed arguments; supported function name/length metadata; simple assignment expressions; arithmetic compound assignments (+=, -=, *=, /=, %=); prefix and postfix numeric updates (++, --); conditional expressions; typeof and void; document-scoped element references from getElementById; current permitted input-value reads and text-content writes; ordered callable click listeners on explicit type=button controls without default actions; listener removal and a bounded click-event facade; parameters and returns; blocks, if/else, bounded while and standard for loops with captured iteration bindings and break/continue; arithmetic, strict equality, abstract equality (==, !=) for supported primitives and reference fast paths, comparisons and short-circuit logic; input reads and textContent writes"
        );
        result["limits"]["statement_nesting"] = json!(STATEMENT_DEPTH);
        result["limits"]["static_text_node_bytes"] =
            json!(crate::constants::STATIC_TEXT_NODE_BYTES);
        result["limits"]["static_text_total_bytes"] =
            json!(crate::constants::STATIC_TEXT_TOTAL_BYTES);
        result["limits"]["layout_work"] = json!(crate::constants::LAYOUT_WORK);
        result["limits"]["vm_environments"] = json!(crate::constants::VM_ENVIRONMENTS);
        result["limits"]["vm_functions"] = json!(crate::constants::VM_FUNCTIONS);
        result["limits"]["vm_captured_bindings"] = json!(crate::constants::VM_BINDINGS);
        result["limits"]["function_nesting"] = json!(crate::constants::FUNCTION_NESTING);
        result["limits"]["vm_event_listeners"] = json!(crate::constants::JS_EVENT_LISTENERS);
        result["limits"]["vm_event_records"] = json!(crate::constants::JS_EVENT_RECORDS);
        if self.profile_store.is_some() {
            result["session_profile"] = json!({"state":if self.profile_error.is_some(){"recovery_required"}else{"ready"},"persistence":"macos_keychain","cookie_management":"native_menu_only"});
        }
        result["operations"]["tab.list"] = json!({});
        result["operations"]["tab.open"] = json!({"human_only":true});
        result["operations"]["tab.select"] = json!({"tab_id":"string","human_only":true});
        result["operations"]["tab.close"] = json!({"tab_id":"string","human_only":true});
        result["tab_limit"] = json!(crate::constants::TABS);
        result["tabs"] = self.tabs_value()?;
        result["operations"]["page.click"] =
            json!({"page_id":"string","node_id":"string","expected_revision":"u64"});
        result["operations"]["page.type"] = json!({"page_id":"string","node_id":"string","expected_revision":"u64","text":"string"});
        result["operations"]["page.set_checked"] = json!({"page_id":"string","node_id":"string","expected_revision":"u64","checked":"bool"});
        result["operations"]["page.select"] = json!({"page_id":"string","node_id":"string","expected_revision":"u64","option_id":"string"});
        result["operations"]["page.submit"] =
            json!({"page_id":"string","node_id":"string","expected_revision":"u64"});
        for operation in [
            "page.back",
            "page.forward",
            "page.reload",
            "control.status",
            "control.release",
        ] {
            result["operations"][operation] = json!({});
        }
        result["operations"]["control.status"] = json!({"tab_id":"optional string"});
        result["operations"]["control.release"] = json!({"tab_id":"string","control_epoch":"u64"});
        if self.web_origins.is_some() {
            result["desktop"] = json!(true);
            result["ownership"] = self.control_status(json!({}))?;
            result["subsets"]["network"] = json!(
                "public HTTP/HTTPS with verified TLS over HTTP/1.1 and HTTP/2; bounded redirects; exact owned fixture grants; public-suffix-aware domain cookies with SameSite (RFC 6265bis) and HttpOnly"
            );
            result["subsets"]["layout"] = json!("logical_points; one shared native display list");
            result["subsets"]["css"] = json!(
                "Owned row/column flex layout with row wrapping, pixel gaps, bounded growth/shrinkage and alignment; explicit content-box and border-box sizing; bounded font shorthand, line-height, vh lengths, block auto margins, HTML/BODY formatting and light/dark canvas colors."
            );
            result["limits"]["layout_work_budget"] = json!(crate::constants::LAYOUT_WORK);
            result["limits"]["layout_extent"] = json!(crate::constants::LAYOUT_EXTENT);
            result["mutation_precondition"] =
                json!("control_epoch from control.status is required on every agent mutation");
            result["unavailable"] = json!([
                "full_css",
                "full_javascript",
                "frames",
                "named_profiles",
                "electron_profile_import",
                "cookie_inspection",
                "os_sandbox"
            ]);
            result["operations"]["page.scroll"] =
                json!({"page_id":"string","expected_revision":"u64","y":"logical_points"});
        }
        result["operations"]["page.inspect"] = json!({"fields":"optional [string]","visible_only":"optional bool","compact":"optional bool"});
        result["operations"]["page.summary"] = json!({});
        result["operations"]["page.find"] = json!({"role":"optional string","name":"optional string","label":"optional string","text":"optional string","limit":"optional u64","visible_only":"optional bool"});
        result["operations"]["page.diff"] = json!({"since_revision":"u64"});
        result["operations"]["page.wait"] = json!({"text":"optional string","role":"optional string","name":"optional string","label":"optional string","absent":"optional bool","url":"optional string","after_revision":"optional u64","navigated_from":"optional string","timeout_ms":"optional u64"});
        result["operations"]["page.act"] = json!({"page_id":"string","expected_revision":"u64","steps":"[step]","result":"optional summary|diff|none","max_steps":super::batch::ACT_STEPS});
        for operation in [
            "page.summary",
            "page.find",
            "page.diff",
            "page.wait",
            "page.act",
            "page.navigate",
            "page.inspect",
            "page.layout",
            "page.set_text",
            "page.click",
            "page.type",
            "page.back",
            "page.forward",
            "page.reload",
            "page.scroll",
            "page.set_checked",
            "page.select",
            "page.submit",
        ] {
            result["operations"][operation]["tab_id"] = json!("string");
        }
        lanes::describe(&mut result);
        super::devtools::describe(&mut result);
        Ok(result)
    }

    /** Shutdown is idempotent; the protocol loop exits after writing its acknowledgment. */
    fn shutdown(&mut self, params: Value) -> EngineResult<Value> {
        let _: Empty = parameters(params)?;
        let session_saved = self.stop();
        Ok(json!({"stopped":true,"session_saved":session_saved}))
    }
}

/** Only successful operations that can change committed tabs or destinations save the session. */
fn session_mutation(op: &str) -> bool {
    [
        "tab.open",
        "tab.select",
        "tab.close",
        "page.navigate",
        "page.click",
        "page.back",
        "page.forward",
        "page.reload",
        "page.submit",
        "page.type",
        "page.set_text",
        "page.set_checked",
        "page.select",
        "page.act",
        "page.click_at",
        "page.press",
        "page.devtools",
        "page.navigate_intercepted",
    ]
    .contains(&op)
}

/// Signed-in network effects remain blocked until profile recovery succeeds.
fn requires_profile(op: &str) -> bool {
    [
        "page.navigate",
        "page.back",
        "page.forward",
        "page.reload",
        "page.click",
        "page.submit",
        "page.click_at",
        "page.press",
        "page.devtools",
        "page.navigate_intercepted",
    ]
    .contains(&op)
}

/// The authority boundary is explicit rather than inferred from an arbitrary prefix.
pub fn mutating(op: &str) -> bool {
    [
        "page.navigate",
        "page.set_text",
        "page.click",
        "page.type",
        "page.back",
        "page.forward",
        "page.reload",
        "page.scroll",
        "page.set_checked",
        "page.select",
        "page.submit",
        "page.act",
        "page.click_at",
        "page.scroll_at",
        "page.press",
        "page.devtools",
        "page.set_attribute",
        "page.scroll_into_view",
        "page.cookies_set",
        "page.cookies_delete",
        "page.cookies_clear",
        "page.emulate",
        "page.navigate_intercepted",
        "runtime.shutdown",
        "control.release",
        "tab.open",
        "tab.select",
        "tab.close",
    ]
    .contains(&op)
}

/// Page handlers retain strict schemas after the routing field is consumed.
fn remove_tab_id(params: &mut Value) {
    if let Some(values) = params.as_object_mut() {
        values.remove("tab_id");
    }
}

/// Successful page results identify the tab whose document they describe.
fn add_tab_id(mut result: Value, id: &str) -> EngineResult<Value> {
    let object = result
        .as_object_mut()
        .ok_or_else(|| EngineError::invalid("A page result was not an object."))?;
    object.insert("tab_id".into(), json!(id));
    Ok(result)
}

/// Agent stdout contains only responses; EOF and failed output retire owned children.
pub fn run_agent(
    runtime: &mut Runtime,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> EngineResult<()> {
    let result = connection(runtime, input, output);
    runtime.stop();
    result
}

/** Cleanup remains outside this loop so every early transport error retires children. */
fn connection(
    runtime: &mut Runtime,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> EngineResult<()> {
    loop {
        let frame = match read_frame(input) {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(error) => {
                write_message(output, &Response::new(None, Err(error)))?;
                break;
            }
        };
        let (response, shutdown) = respond(runtime, &frame);
        write_message(output, &response)?;
        if shutdown {
            break;
        }
    }
    Ok(())
}

/** Recover correlation only when it is a valid bounded ID; malformed JSON gets null. */
fn respond(runtime: &mut Runtime, frame: &[u8]) -> (Response, bool) {
    let id = decode::<Value>(frame).ok().and_then(|value| {
        value
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| valid_id(id))
            .map(str::to_owned)
    });
    let request = decode::<Request>(frame);
    let shutdown = request
        .as_ref()
        .is_ok_and(|request| request.op == "runtime.shutdown");
    let result = request.and_then(|request| runtime.handle(request));
    let shutdown = shutdown && result.is_ok();
    (Response::new(id, result), shutdown)
}
