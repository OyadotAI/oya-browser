/**
 * Oya's command vocabulary, run as Mars native operations. The Oya server sends `cmd` messages in
 * the Electron browser's words (`navigate`, `analyze`, `click` on `[data-ac-id="N"]`, ...); each
 * one here becomes a Mars `Request` that the integration runs through the runtime as the agent, so
 * Mars's own human/agent ownership, control epochs and expected revisions decide every write.
 * Nothing here grants control, picks a tab the agent was not given, or reaches a page script.
 */
use mars_core::protocol::{EngineError, EngineResult, Request};
use serde_json::{Map, Value, json};

/// The browser the link drives: Mars's runtime, admitted as the connected agent.
pub trait Browser {
    /// Runs one native operation as the agent. The implementation admits it with an agent permit
    /// (adding `control_epoch`) so the runtime refuses it whenever the person holds the tab.
    fn run(&mut self, request: Request) -> EngineResult<Value>;
    /// A PNG of one tab's current display list, refused unless the agent may observe that tab.
    fn screenshot(&mut self, tab_id: &str) -> EngineResult<Vec<u8>>;
    /// The server put the person in control: revoke every agent permit now. Never grants.
    fn revoke_agents(&mut self);
}

/// The actions this link answers, announced in `auth` so the server routes only these by name.
pub const ACTIONS: [&str; 13] = [
    "analyze",
    "back",
    "click",
    "close_tab",
    "forward",
    "list_tabs",
    "navigate",
    "open_tab",
    "reload",
    "screenshot",
    "switch_tab",
    "type",
    "read_page",
];

/// A failed command: the text the server relays, and one of Oya's result codes when it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    /// The error text the caller sees.
    pub message: String,
    /// `action_unsupported` and the like; only codes Oya's server knows.
    pub code: Option<&'static str>,
}

impl From<EngineError> for CommandError {
    fn from(error: EngineError) -> Self {
        Self {
            message: error.message,
            code: None,
        }
    }
}

/// A plain failure.
fn fail<T>(message: impl Into<String>) -> Result<T, CommandError> {
    Err(CommandError {
        message: message.into(),
        code: None,
    })
}

/// The last `analyze` of the agent's tab: which node each numbered element is, on which revision.
struct Analysis {
    /// The tab it was read from.
    tab: String,
    /// The page generation.
    page_id: String,
    /// The revision the numbers belong to.
    revision: u64,
    /// Native node id by element number - 1.
    nodes: Vec<String>,
}

/// One connection's command state: the agent's tab and its last analysis.
#[derive(Default)]
pub struct Commands {
    /// The tab the agent works in; the selected tab until switch_tab names another.
    tab: Option<String>,
    /// The element numbers the agent was last given.
    analysis: Option<Analysis>,
    /// Correlation ids for native requests.
    sequence: u64,
}

/// One action's handler.
type Handler = fn(&mut Commands, &mut dyn Browser, &Value) -> Result<Value, CommandError>;

/// Action name to handler; a name not here is answered `action_unsupported`.
const HANDLERS: [(&str, Handler); 13] = [
    ("navigate", Commands::navigate),
    ("analyze", Commands::analyze),
    ("read_page", Commands::analyze),
    ("click", Commands::click),
    ("type", Commands::type_text),
    ("back", |c, b, _| c.page_op(b, "page.back", Map::new())),
    ("forward", |c, b, _| {
        c.page_op(b, "page.forward", Map::new())
    }),
    ("reload", |c, b, _| c.page_op(b, "page.reload", Map::new())),
    ("screenshot", Commands::screenshot),
    ("list_tabs", Commands::list_tabs),
    ("switch_tab", Commands::switch_tab),
    ("open_tab", Commands::open_tab),
    ("close_tab", Commands::close_tab),
];

impl Commands {
    /// Runs one server command and returns its result data.
    pub fn run(
        &mut self,
        browser: &mut dyn Browser,
        action: &str,
        params: &Value,
    ) -> Result<Value, CommandError> {
        let Some((_, handler)) = HANDLERS.iter().find(|(name, _)| *name == action) else {
            let message = format!("The action \"{action}\" is not supported on Mars yet.");
            return Err(CommandError {
                message,
                code: Some("action_unsupported"),
            });
        };
        handler(self, browser, params)
    }

    /// A native request with a fresh correlation id.
    fn request(&mut self, op: &str, params: Value) -> Request {
        self.sequence += 1;
        Request::new(&format!("link-{}", self.sequence), op, params)
    }

    /// The agent's tab: the one it switched to, else the selected one.
    fn tab(&mut self, browser: &mut dyn Browser) -> Result<String, CommandError> {
        if let Some(tab) = &self.tab {
            return Ok(tab.clone());
        }
        let tabs = browser.run(self.request("tab.list", json!({})))?;
        let selected = tabs
            .get("selected_tab_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        selected.map_or_else(|| fail("No active tab"), Ok)
    }

    /// A page operation on the agent's tab; any page change invalidates the element numbers.
    fn page_op(
        &mut self,
        browser: &mut dyn Browser,
        op: &str,
        mut params: Map<String, Value>,
    ) -> Result<Value, CommandError> {
        if !params.contains_key("tab_id") {
            params.insert("tab_id".into(), self.tab(browser)?.into());
        }
        let result = browser.run(self.request(op, Value::Object(params)));
        if op != "page.summary" {
            self.analysis = None;
        }
        Ok(result?)
    }

    /// `navigate {url}`.
    fn navigate(
        &mut self,
        browser: &mut dyn Browser,
        params: &Value,
    ) -> Result<Value, CommandError> {
        let Some(url) = params.get("url").and_then(Value::as_str) else {
            return fail("url must be a string");
        };
        let result = self.page_op(
            browser,
            "page.navigate",
            Map::from_iter([("url".into(), url.into())]),
        )?;
        Ok(
            json!({"url": result.get("url").cloned().unwrap_or(url.into()), "title": result.get("title").cloned().unwrap_or_default()}),
        )
    }

    /// `analyze`: Mars's page summary, its controls numbered as Oya's elements.
    fn analyze(&mut self, browser: &mut dyn Browser, _: &Value) -> Result<Value, CommandError> {
        let tab = self.tab(browser)?;
        let summary = self.page_op(browser, "page.summary", Map::new())?;
        let controls = summary
            .get("controls")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let nodes = controls
            .iter()
            .map(|c| {
                c.get("node_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            })
            .collect();
        let page_id = summary
            .get("page_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let revision = summary
            .get("revision")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        self.analysis = Some(Analysis {
            tab,
            page_id,
            revision,
            nodes,
        });
        Ok(analysis_value(&summary, &controls))
    }

    /// The tab, page, node and revision an element number names, from the last analysis.
    fn target(&self, params: &Value) -> Result<(String, String, String, u64), CommandError> {
        let number = element_number(params).ok_or_else(|| CommandError {
            message: "Name the element by its analyze_page id.".into(),
            code: None,
        })?;
        let Some(analysis) = &self.analysis else {
            return fail("The page changed since analyze_page. Call analyze_page again.");
        };
        let node = number
            .checked_sub(1)
            .and_then(|i| analysis.nodes.get(i))
            .filter(|n| !n.is_empty());
        let node = node.ok_or_else(|| CommandError {
            message: format!("Element {number} is not on the page. Call analyze_page again."),
            code: None,
        })?;
        Ok((
            analysis.tab.clone(),
            analysis.page_id.clone(),
            node.clone(),
            analysis.revision,
        ))
    }

    /// An element action on the analysed revision; Mars refuses it if the page moved on.
    fn element_op(
        &mut self,
        browser: &mut dyn Browser,
        op: &str,
        params: &Value,
        extra: Option<(&str, Value)>,
    ) -> Result<Value, CommandError> {
        let (tab, page_id, node_id, revision) = self.target(params)?;
        let fields = [
            ("tab_id", tab.into()),
            ("page_id", page_id.into()),
            ("node_id", node_id.into()),
            ("expected_revision", revision.into()),
        ];
        let mut args: Map<String, Value> =
            fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
        args.extend(extra.map(|(k, v)| (k.to_owned(), v)));
        self.page_op(browser, op, args)
    }

    /// `click {selector: [data-ac-id="N"]}`.
    fn click(&mut self, browser: &mut dyn Browser, params: &Value) -> Result<Value, CommandError> {
        self.element_op(browser, "page.click", params, None)
    }

    /// `type {selector, text}`: replaces the field's text.
    fn type_text(
        &mut self,
        browser: &mut dyn Browser,
        params: &Value,
    ) -> Result<Value, CommandError> {
        let Some(text) = params.get("text").and_then(Value::as_str) else {
            return fail("text must be a string");
        };
        self.element_op(browser, "page.type", params, Some(("text", text.into())))
    }

    /// `screenshot {tab_id?}`: a PNG data URL, as Oya's `screenshot` result carries it.
    fn screenshot(
        &mut self,
        browser: &mut dyn Browser,
        params: &Value,
    ) -> Result<Value, CommandError> {
        let tab = match params.get("tab_id") {
            Some(id) => mars_tab(id).ok_or_else(|| CommandError {
                message: "Invalid tab_id".into(),
                code: None,
            })?,
            None => self.tab(browser)?,
        };
        let png = browser.screenshot(&tab)?;
        Ok(json!({"screenshot": format!("data:image/png;base64,{}", crate::base64::encode(&png))}))
    }

    /// `list_tabs`: Oya's numeric ids, the agent's tab marked active, and which tabs it may use.
    fn list_tabs(&mut self, browser: &mut dyn Browser, _: &Value) -> Result<Value, CommandError> {
        let mine = self.tab(browser)?;
        let listed = browser.run(self.request("tab.list", json!({})))?;
        let tabs = listed
            .get("tabs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let tabs = tabs.iter().filter_map(|tab| {
            let id = tab.get("tab_id").and_then(Value::as_str)?;
            let yours = tab.get("owner").and_then(Value::as_str) == Some("agent");
            Some(json!({"id": oya_tab(id)?, "title": tab.get("title").cloned().unwrap_or_default(), "url": "", "active": id == mine, "yours": yours}))
        });
        Ok(json!({"tabs": tabs.collect::<Vec<_>>()}))
    }

    /// `switch_tab {tab_id}`: the agent works there from now on; the person's view does not move,
    /// and Mars still refuses every write in a tab the person holds.
    fn switch_tab(
        &mut self,
        browser: &mut dyn Browser,
        params: &Value,
    ) -> Result<Value, CommandError> {
        let tab = params
            .get("tab_id")
            .and_then(mars_tab)
            .ok_or_else(|| CommandError {
                message: "Invalid tab_id".into(),
                code: None,
            })?;
        let listed = browser.run(self.request("tab.list", json!({})))?;
        let tabs = listed
            .get("tabs")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let entry = tabs
            .iter()
            .find(|t| t.get("tab_id").and_then(Value::as_str) == Some(tab.as_str()));
        let Some(entry) = entry else {
            return fail("No such tab. Use list_tabs.");
        };
        if entry.get("owner").and_then(Value::as_str) != Some("agent") {
            return fail("Not switched: that tab belongs to the person. Work in your own tab.");
        }
        self.tab = Some(tab);
        self.analysis = None;
        Ok(json!({"tab_id": params["tab_id"]}))
    }

    /// `open_tab {url?}`: Mars opens tabs for the person only until agent-owned tabs land (AG-10),
    /// so the runtime's refusal is what the caller hears.
    fn open_tab(&mut self, browser: &mut dyn Browser, _: &Value) -> Result<Value, CommandError> {
        let opened = browser.run(self.request("tab.open", json!({})))?;
        Ok(opened)
    }

    /// `close_tab {tab_id}`: as open_tab, decided by the runtime's ownership rules.
    fn close_tab(
        &mut self,
        browser: &mut dyn Browser,
        params: &Value,
    ) -> Result<Value, CommandError> {
        let tab = match params.get("tab_id") {
            Some(id) => mars_tab(id).ok_or_else(|| CommandError {
                message: "Invalid tab_id".into(),
                code: None,
            })?,
            None => self.tab(browser)?,
        };
        Ok(browser.run(self.request("tab.close", json!({"tab_id": tab})))?)
    }
}

/// The element number in `element_id`, `element` or a `[data-ac-id="N"]` selector.
fn element_number(params: &Value) -> Option<usize> {
    let direct = ["element_id", "element"]
        .iter()
        .find_map(|k| params.get(*k).and_then(Value::as_u64));
    let selector = params
        .get("selector")
        .and_then(Value::as_str)
        .and_then(|s| {
            let inner = s.strip_prefix("[data-ac-id=\"")?.strip_suffix("\"]")?;
            inner.parse::<u64>().ok()
        });
    direct.or(selector).and_then(|n| usize::try_from(n).ok())
}

/// Oya's numeric tab id for a Mars `tab-N`.
fn oya_tab(id: &str) -> Option<u64> {
    id.strip_prefix("tab-")?.parse().ok()
}

/// Mars's `tab-N` for an Oya numeric (or already Mars-shaped) tab id.
fn mars_tab(id: &Value) -> Option<String> {
    match id {
        Value::Number(n) => n.as_u64().filter(|n| *n > 0).map(|n| format!("tab-{n}")),
        Value::String(s) if oya_tab(s).is_some() => Some(s.clone()),
        _ => None,
    }
}

/// Oya's element type for a Mars control role.
fn element_type(role: &str) -> &str {
    match role {
        "textbox" | "searchbox" => "input",
        "combobox" | "listbox" => "select",
        other => other,
    }
}

/// One Mars control as an Oya page element.
fn element(control: &Value) -> Value {
    let field = |k: &str| control.get(k).cloned();
    let role = control
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut out = json!({"id": field("n").unwrap_or_default(), "type": element_type(role), "text": field("name").unwrap_or_else(|| "".into()), "visible": true});
    for key in ["value", "placeholder", "checked", "disabled"] {
        if let Some(value) = field(key) {
            out[key] = value;
        }
    }
    if let Some(message) = field("invalid") {
        out["invalid"] = true.into();
        out["error"] = message;
    }
    if let Some(options) = control.get("options").and_then(Value::as_array) {
        let texts: Vec<&str> = options
            .iter()
            .filter_map(|o| o.get("text").and_then(Value::as_str))
            .collect();
        out["options"] = texts.join(", ").into();
    }
    out
}

/// The `analyze` result: url, title, a short markdown outline and the numbered elements.
fn analysis_value(summary: &Value, controls: &[Value]) -> Value {
    let text = |k: &str| {
        summary
            .get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let mut markdown = format!("# {}\n", text("title"));
    outline(summary.get("headings"), &mut markdown);
    let elements: Vec<Value> = controls.iter().map(element).collect();
    json!({"url": text("url"), "title": text("title"), "markdown": markdown, "elements": elements, "page_id": text("page_id"), "revision": summary.get("revision").cloned().unwrap_or_default()})
}

/// Headings as markdown lines, depth first.
fn outline(headings: Option<&Value>, out: &mut String) {
    for heading in headings.and_then(Value::as_array).into_iter().flatten() {
        let level = heading
            .get("level")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .clamp(1, 6) as usize;
        let text = heading
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        out.push_str(&format!("\n{} {text}\n", "#".repeat(level)));
        outline(heading.get("children"), out);
    }
}
