/**
 * Runtime, Console and Log domains: evaluation against the page's realm through V9's devtools
 * entry (Runtime.evaluate, callFunctionOn, getProperties, releaseObject, awaitPromise), remote
 * object ids spelled as V8's injected script spells them, and the console and exception events
 * a page's output produces. A connection that holds the tab commits what its scripts change; one
 * that does not reads the realm in place, as `page.devtools_peek` allows.
 */
use super::{Fault, Handler, Session};
use serde_json::{Value, json};

/// Chrome's error for an evaluation in a context that no longer exists.
const NO_CONTEXT: &str = "Cannot find context with specified id";
/// Console levels V9 reports, mapped to Runtime.consoleAPICalled types.
const LEVELS: &[(&str, &str)] = &[
    ("log", "log"),
    ("info", "info"),
    ("warn", "warning"),
    ("error", "error"),
    ("debug", "debug"),
];

/** The Runtime commands. */
pub(super) const HANDLERS: &[(&str, Handler)] = &[
    ("Runtime.enable", Session::runtime_enable),
    ("Runtime.evaluate", Session::evaluate),
    ("Runtime.callFunctionOn", Session::call_function),
    ("Runtime.getProperties", Session::get_properties),
    ("Runtime.releaseObject", Session::release_object),
    ("Runtime.releaseObjectGroup", Session::release_group),
    ("Runtime.awaitPromise", Session::await_promise),
];

/** A parsed remote object id: the context it belongs to and V9's handle. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ObjectId {
    pub context: u64,
    pub handle: u32,
}

impl ObjectId {
    /** The spelling V8's injected script uses. */
    pub fn text(self) -> String {
        format!(
            "{{\"injectedScriptId\":{},\"id\":{}}}",
            self.context, self.handle
        )
    }
    /** Parse the spelling back; anything else is an invalid id. */
    pub fn parse(text: &str) -> Result<Self, Fault> {
        let value: Value =
            serde_json::from_str(text).map_err(|_| Fault::invalid("Invalid remote object id"))?;
        let context = value["injectedScriptId"].as_u64();
        let handle = value["id"].as_u64().and_then(|id| u32::try_from(id).ok());
        match (context, handle) {
            (Some(context), Some(handle)) => Ok(Self { context, handle }),
            _ => Err(Fault::invalid("Invalid remote object id")),
        }
    }
}

/** Mirror options from the command's parameters. */
fn options(params: &Value) -> Value {
    json!({
        "return_by_value": params["returnByValue"].as_bool().unwrap_or(false),
        "preview": params["generatePreview"].as_bool().unwrap_or(false),
        "await_promise": params["awaitPromise"].as_bool().unwrap_or(false),
        "group": params["objectGroup"].as_str().unwrap_or_default(),
    })
}

impl Session {
    /** Runtime.enable announces the current document's main world and its isolated worlds. */
    fn runtime_enable(&mut self, session: Option<&str>, _: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let url = self.current_url(&tab);
        let (context, worlds) = self
            .targets
            .get(&tab)
            .map(|target| (target.context, target.worlds.clone()))
            .unwrap_or_default();
        self.emit(
            Some(&tab),
            "Runtime.executionContextCreated",
            json!({"context":Self::context_description(context, &tab, &url, "", true)}),
        );
        for (id, name) in worlds {
            self.emit(
                Some(&tab),
                "Runtime.executionContextCreated",
                json!({"context":Self::context_description(id, &tab, &url, &name, false)}),
            );
        }
        Ok(json!({}))
    }

    /** The tab an evaluation targets: its contextId, its objectId, or the session's tab. */
    fn evaluation_tab(
        &self,
        session: Option<&str>,
        params: &Value,
    ) -> Result<(String, u64), Fault> {
        if let Some(context) = params["contextId"]
            .as_u64()
            .or_else(|| params["executionContextId"].as_u64())
        {
            let tab = self
                .contexts
                .get(&context)
                .ok_or_else(|| Fault::invalid(NO_CONTEXT))?;
            return Ok((tab.clone(), context));
        }
        if let Some(text) = params["objectId"].as_str() {
            let id = ObjectId::parse(text)?;
            let tab = self
                .contexts
                .get(&id.context)
                .ok_or_else(|| Fault::invalid(NO_CONTEXT))?;
            return Ok((tab.clone(), id.context));
        }
        let tab = self.tab(session)?;
        let context = self.targets.get(&tab).map_or(0, |target| target.context);
        Ok((tab, context))
    }

    /**
     * One devtools request on a tab: under control it commits; refused for want of control, it
     * is re-run as a peek, so reads keep working on a tab the person holds. Remote object ids in
     * the answer are rewritten for this context, and node handles are remembered for DOM.
     */
    pub(super) fn devtools(&mut self, tab: &str, request: Value) -> Result<Value, Fault> {
        let result = match self.mutate(tab, "page.devtools", json!({"request":request.clone()})) {
            Err(fault) if fault.denied() => self
                .call_tab(tab, "page.devtools_peek", json!({"request":request}))
                .map_err(|peek| peeked(&fault, peek)),
            result => result,
        }?;
        if let Some(navigated) = result.get("navigated").cloned() {
            let page = self
                .call_tab(tab, "page.wait", json!({"after_revision":0}))
                .unwrap_or(navigated);
            self.navigated(tab, &page);
        }
        Ok(result)
    }

    /** Rewrite V9's handles into remote object ids for `context`, recording node handles. */
    pub(super) fn adopt_mirror(&mut self, value: &mut Value, context: u64, tab: &str) {
        match value {
            Value::Object(fields) => {
                let node = fields.remove("marsNode").and_then(|node| node.as_u64());
                if let Some(Value::String(handle)) = fields.get("objectId")
                    && let Ok(handle) = handle.parse::<u32>()
                {
                    let id = ObjectId { context, handle }.text();
                    if let (Some(node), Some(target)) = (node, self.targets.get_mut(tab)) {
                        target.nodes.insert(id.clone(), node as usize);
                    }
                    fields.insert("objectId".into(), json!(id));
                }
                for (_, field) in fields.iter_mut() {
                    self.adopt_mirror(field, context, tab);
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.adopt_mirror(item, context, tab);
                }
            }
            _ => {}
        }
    }

    /** Console lines a request produced become consoleAPICalled and exceptionThrown events. */
    fn report_entries(&mut self, tab: &str, entries: &[Value], context: u64) {
        for entry in entries {
            let level = entry["level"].as_str().unwrap_or("log");
            let text = entry["text"].as_str().unwrap_or_default().to_owned();
            let timestamp = self.timestamp();
            if level == "error" && text.starts_with("Uncaught") {
                self.next_request += 1;
                self.emit(Some(tab), "Runtime.exceptionThrown", json!({"timestamp":timestamp,"exceptionDetails":{"exceptionId":self.next_request,"text":text,"lineNumber":0,"columnNumber":0,"executionContextId":context,"exception":{"type":"object","subtype":"error","className":"Error","description":text}}}));
                continue;
            }
            let kind = LEVELS
                .iter()
                .find(|(name, _)| *name == level)
                .map_or("log", |(_, kind)| kind);
            self.emit(Some(tab), "Runtime.consoleAPICalled", json!({"type":kind,"args":[{"type":"string","value":text}],"executionContextId":context,"timestamp":timestamp}));
        }
    }

    /** Report the page's own console lines this connection has not seen yet. */
    pub(super) fn report_console(&mut self, tab: &str) {
        let Ok(console) = self.call_tab(tab, "page.console", json!({})) else {
            return;
        };
        let dropped = console["page_dropped"].as_u64().unwrap_or(0) as usize;
        let entries: Vec<Value> = console["entries"]
            .as_array()
            .map(|entries| {
                entries
                    .iter()
                    .filter(|entry| entry["source"] == "page")
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let seen = self
            .targets
            .get(tab)
            .map_or(0, |target| target.console_seen);
        let skip = seen.saturating_sub(dropped).min(entries.len());
        let fresh: Vec<Value> = entries[skip..].to_vec();
        if let Some(target) = self.targets.get_mut(tab) {
            target.console_seen = dropped + entries.len();
        }
        let context = self.targets.get(tab).map_or(0, |target| target.context);
        self.report_entries(tab, &fresh, context);
    }

    /** Finish an evaluation: events for its console lines, ids rewritten, Chrome's result shape. */
    fn finish(&mut self, tab: &str, context: u64, mut result: Value) -> Result<Value, Fault> {
        let entries = result["console"].as_array().cloned().unwrap_or_default();
        self.report_entries(tab, &entries, context);
        if result["pending"] == true {
            return Err(Fault::server(
                "Promise was not settled: the page's work did not finish in time.",
            ));
        }
        let mut answer = json!({"result":result["result"].take()});
        if let Some(details) = result.get("exceptionDetails").cloned() {
            answer["exceptionDetails"] = details;
        }
        self.adopt_mirror(&mut answer, context, tab);
        Ok(answer)
    }

    /** Runtime.evaluate. */
    fn evaluate(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let (tab, context) = self.evaluation_tab(session, params)?;
        let expression = params["expression"].as_str().unwrap_or_default();
        let result = self.devtools(
            &tab,
            json!({"kind":"evaluate","expression":expression,"options":options(params)}),
        )?;
        self.finish(&tab, context, result)
    }

    /** Runtime.callFunctionOn. */
    fn call_function(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let (tab, context) = self.evaluation_tab(session, params)?;
        let declaration = params["functionDeclaration"].as_str().unwrap_or_default();
        let this = match params["objectId"].as_str() {
            Some(text) => Some(ObjectId::parse(text)?.handle),
            None => None,
        };
        let mut arguments = Vec::new();
        for argument in params["arguments"].as_array().cloned().unwrap_or_default() {
            arguments.push(self.call_argument(&tab, &argument)?);
        }
        let request = json!({"kind":"call_function","declaration":declaration,"this":this,"arguments":arguments,"options":options(params)});
        let result = self.devtools(&tab, request)?;
        self.finish(&tab, context, result)
    }

    /** A Runtime.CallArgument for V9; an element argument is remembered as the one touched. */
    fn call_argument(&mut self, tab: &str, argument: &Value) -> Result<Value, Fault> {
        if let Some(text) = argument["objectId"].as_str() {
            let id = ObjectId::parse(text)?;
            if let Some(target) = self.targets.get_mut(tab)
                && let Some(node) = target.nodes.get(text).copied()
            {
                target.touched = Some(node);
            }
            return Ok(json!({"handle":id.handle}));
        }
        if let Some(text) = argument["unserializableValue"].as_str() {
            return Ok(json!({"unserializable":text}));
        }
        Ok(json!({"value":argument.get("value").cloned().unwrap_or(Value::Null)}))
    }

    /** Runtime.getProperties. */
    fn get_properties(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let (tab, context) = self.evaluation_tab(session, params)?;
        let id = ObjectId::parse(params["objectId"].as_str().unwrap_or_default())?;
        let request = json!({"kind":"get_properties","handle":id.handle,"own_only":params["ownProperties"].as_bool().unwrap_or(false),"accessors_only":params["accessorPropertiesOnly"].as_bool().unwrap_or(false),"options":{"preview":params["generatePreview"].as_bool().unwrap_or(false)}});
        let mut result = self.devtools(&tab, request)?;
        let mut answer = json!({"result":result["result"].take()});
        self.adopt_mirror(&mut answer, context, &tab);
        Ok(answer)
    }

    /** Runtime.releaseObject. */
    fn release_object(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let (tab, _) = self.evaluation_tab(session, params)?;
        let id = ObjectId::parse(params["objectId"].as_str().unwrap_or_default())?;
        if let Some(target) = self.targets.get_mut(&tab) {
            target
                .nodes
                .remove(params["objectId"].as_str().unwrap_or_default());
        }
        let _ = self.devtools(&tab, json!({"kind":"release","handles":[id.handle]}));
        Ok(json!({}))
    }

    /** Runtime.releaseObjectGroup. */
    fn release_group(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = self.tab(session)?;
        let group = params["objectGroup"].as_str().unwrap_or_default();
        let _ = self.devtools(&tab, json!({"kind":"release_group","group":group}));
        Ok(json!({}))
    }

    /** Runtime.awaitPromise. */
    fn await_promise(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let id = ObjectId::parse(params["promiseObjectId"].as_str().unwrap_or_default())?;
        let tab = self
            .contexts
            .get(&id.context)
            .cloned()
            .ok_or_else(|| Fault::invalid(NO_CONTEXT))?;
        let _ = session;
        let request = json!({"kind":"await","handle":id.handle,"options":{"return_by_value":params["returnByValue"].as_bool().unwrap_or(false),"preview":params["generatePreview"].as_bool().unwrap_or(false),"await_promise":true}});
        let result = self.devtools(&tab, request)?;
        self.finish(&tab, id.context, result)
    }
}

/** A peek that failed after control was refused says so: handles from a session without control
do not outlive their call, which is what a client missing an object id is running into. */
fn peeked(denied: &Fault, peek: Fault) -> Fault {
    if peek.message.contains("Could not find object") {
        return Fault::server(&format!(
            "{} Remote objects do not persist without control; evaluate again once the person hands the tab back.",
            denied.message
        ));
    }
    peek
}

#[cfg(test)]
mod tests {
    use super::ObjectId;

    /** Remote object ids round-trip through V8's spelling. */
    #[test]
    fn object_ids_round_trip() {
        let id = ObjectId {
            context: 3,
            handle: 7,
        };
        assert_eq!(id.text(), "{\"injectedScriptId\":3,\"id\":7}");
        assert_eq!(ObjectId::parse(&id.text()).unwrap(), id);
        assert!(ObjectId::parse("7").is_err());
    }
}
