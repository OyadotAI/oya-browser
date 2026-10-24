/**
 * Target and Browser domains: discovery, flattened attachment (sessionId multiplexing), tabs
 * opened and closed under this connection's lane, and the browser's version and window. The
 * semantics follow content/browser/devtools/protocol/target_handler.cc and browser_handler.cc:
 * browser-level auto-attach is flattened only, attached targets report `waitingForDebugger`
 * when the client asked for it, and closing a target the lane does not own is refused.
 */
use super::{Fault, Handler, Session};
use serde_json::{Value, json};

/// Chrome's error for a target id that no longer exists.
const TARGET_NOT_FOUND: &str = "No target with given id found";
/// Chrome's refusal of non-flattened browser-level auto-attach.
const FLATTEN_ONLY: &str = "Only flatten protocol is supported with browser level auto-attach";
/// The one window Mars shows; Chrome's window ids are small integers.
const WINDOW_ID: u64 = 1;

/** The Target and Browser commands. */
pub(super) const HANDLERS: &[(&str, Handler)] = &[
    ("Browser.getVersion", Session::version),
    ("Browser.close", Session::close),
    ("Browser.getWindowForTarget", Session::window_for_target),
    ("Browser.setWindowBounds", Session::set_window_bounds),
    ("Target.setDiscoverTargets", Session::discover),
    ("Target.getTargets", Session::targets),
    ("Target.getTargetInfo", Session::target_info),
    ("Target.setAutoAttach", Session::auto_attach),
    ("Target.attachToTarget", Session::attach),
    ("Target.attachToBrowserTarget", Session::attach_browser),
    ("Target.detachFromTarget", Session::detach),
    ("Target.createTarget", Session::create_target),
    ("Target.closeTarget", Session::close_target),
    ("Target.getBrowserContexts", Session::browser_contexts),
    ("Schema.getDomains", Session::schema_domains),
];

/** The user agent Mars presents to pages and reports as the browser's. */
pub fn user_agent() -> String {
    format!(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) {} Safari/537.36",
        mars::network::CHROME_ENGINE
    )
}

impl Session {
    /** Browser.getVersion: the engine Relay presents, V9 as the JavaScript engine. */
    fn version(&mut self, _: Option<&str>, _: &Value) -> Result<Value, Fault> {
        Ok(json!({
            "protocolVersion": crate::catalog::Catalog::pinned().version,
            "product": mars::network::CHROME_ENGINE,
            "revision": format!("@mars-{}", mars::constants::VERSION),
            "userAgent": user_agent(),
            "jsVersion": "V9",
        }))
    }

    /** Browser.close ends this client's connection; the person's browser keeps running. */
    fn close(&mut self, _: Option<&str>, _: &Value) -> Result<Value, Fault> {
        self.closing = true;
        Ok(json!({}))
    }

    /** Browser.getWindowForTarget: the one window, sized as the viewport. */
    fn window_for_target(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = params["targetId"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| self.tab(session).ok());
        let viewport = tab
            .and_then(|tab| self.call_tab(&tab, "page.nodes", json!({})).ok())
            .map(|nodes| nodes["viewport"].clone())
            .unwrap_or_else(|| json!({"width":mars::constants::DEFAULT_VIEWPORT_WIDTH,"height":mars::constants::DEFAULT_VIEWPORT_HEIGHT}));
        Ok(
            json!({"windowId":WINDOW_ID,"bounds":{"left":0,"top":0,"width":viewport["width"],"height":viewport["height"],"windowState":"normal"}}),
        )
    }

    /** Browser.setWindowBounds: a size change becomes the shared viewport. */
    fn set_window_bounds(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let bounds = &params["bounds"];
        if let (Some(width), Some(height)) = (bounds["width"].as_f64(), bounds["height"].as_f64()) {
            let tab = self.tab(session).or_else(|_| self.any_tab())?;
            self.mutate(
                &tab,
                "page.emulate",
                json!({"viewport":{"width":width,"height":height}}),
            )?;
        }
        Ok(json!({}))
    }

    /** Some tab this connection may act on, for browser-level commands. */
    fn any_tab(&mut self) -> Result<String, Fault> {
        self.targets
            .keys()
            .next()
            .cloned()
            .ok_or_else(|| Fault::server("No target is attached."))
    }

    /** Target info built from the tab list entry plus its current URL. */
    fn info(&mut self, tab: &Value) -> Value {
        let id = tab["tab_id"].as_str().unwrap_or_default().to_owned();
        let url = self
            .call_tab(&id, "page.wait", json!({"after_revision":0}))
            .ok()
            .and_then(|page| page["url"].as_str().map(str::to_owned))
            .unwrap_or_else(|| "about:blank".into());
        let attached = self.sessions.values().any(|tab| *tab == id);
        json!({"targetId":id,"type":"page","title":tab["title"],"url":url,"attached":attached,"canAccessOpener":false,"browserContextId":"default"})
    }

    /** Every tab as target info. */
    fn all_targets(&mut self) -> Result<Vec<Value>, Fault> {
        let tabs = self.call("tab.list", json!({}))?;
        let tabs = tabs["tabs"].as_array().cloned().unwrap_or_default();
        Ok(tabs.iter().map(|tab| self.info(tab)).collect())
    }

    /** Target.setDiscoverTargets announces existing tabs. */
    fn discover(&mut self, _: Option<&str>, params: &Value) -> Result<Value, Fault> {
        if params["discover"].as_bool() == Some(true) {
            for info in self.all_targets()? {
                self.emit(None, "Target.targetCreated", json!({"targetInfo":info}));
            }
        }
        Ok(json!({}))
    }

    /** Target.getTargets. */
    fn targets(&mut self, _: Option<&str>, _: &Value) -> Result<Value, Fault> {
        Ok(json!({"targetInfos":self.all_targets()?}))
    }

    /** Target.getTargetInfo: one tab, or the browser itself. */
    fn target_info(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let id = params["targetId"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| self.tab(session).ok());
        let Some(id) = id else {
            return Ok(
                json!({"targetInfo":{"targetId":"browser","type":"browser","title":"Mars","url":"","attached":true,"canAccessOpener":false}}),
            );
        };
        let info = self
            .all_targets()?
            .into_iter()
            .find(|info| info["targetId"] == id.as_str());
        info.map(|info| json!({"targetInfo":info}))
            .ok_or_else(|| Fault::invalid(TARGET_NOT_FOUND))
    }

    /** Schema.getDomains: the pinned protocol's domains. */
    fn schema_domains(&mut self, _: Option<&str>, _: &Value) -> Result<Value, Fault> {
        let catalog = crate::catalog::Catalog::pinned();
        let domains: Vec<Value> = catalog
            .domains
            .iter()
            .map(|domain| json!({"name":domain.name,"version":catalog.version}))
            .collect();
        Ok(json!({"domains":domains}))
    }

    /** Target.getBrowserContexts: Mars has one profile, Chrome's default context. */
    fn browser_contexts(&mut self, _: Option<&str>, _: &Value) -> Result<Value, Fault> {
        Ok(json!({"browserContextIds":[]}))
    }

    /** Target.setAutoAttach at browser level attaches every tab and every tab created later. */
    fn auto_attach(&mut self, session: Option<&str>, params: &Value) -> Result<Value, Fault> {
        if session.is_some() || self.default_tab.is_some() {
            return Ok(json!({}));
        }
        if params["autoAttach"].as_bool() == Some(true) && params["flatten"].as_bool() != Some(true)
        {
            return Err(Fault::invalid(FLATTEN_ONLY));
        }
        self.auto_attach = params["autoAttach"].as_bool() == Some(true);
        self.wait_for_debugger = params["waitForDebuggerOnStart"].as_bool() == Some(true);
        if self.auto_attach {
            for info in self.all_targets()? {
                let id = info["targetId"].as_str().unwrap_or_default().to_owned();
                if !self.sessions.values().any(|tab| *tab == id) {
                    self.attach_tab(&id, info);
                }
            }
        }
        Ok(json!({}))
    }

    /** Target.attachToTarget with flatten: true. */
    fn attach(&mut self, _: Option<&str>, params: &Value) -> Result<Value, Fault> {
        if params["flatten"].as_bool() != Some(true) {
            return Err(Fault::invalid(
                "Mars supports flattened sessions only (flatten: true); messages are routed through sessionId.",
            ));
        }
        let id = params["targetId"]
            .as_str()
            .ok_or_else(|| Fault::invalid("targetId is required."))?
            .to_owned();
        let info = self
            .all_targets()?
            .into_iter()
            .find(|info| info["targetId"] == id.as_str())
            .ok_or_else(|| Fault::invalid(TARGET_NOT_FOUND))?;
        // Chrome tells the session that asked, not the auto-attach listener on the root.
        let asker = self.current.clone().and_then(|(_, session)| session);
        Ok(json!({"sessionId":self.attach_tab_for(&id, info, asker)}))
    }

    /** Target.attachToBrowserTarget: a session for browser-level commands, as Chrome's. */
    fn attach_browser(&mut self, _: Option<&str>, _: &Value) -> Result<Value, Fault> {
        self.next_session += 1;
        let session = format!("BROWSER{:025X}", self.next_session);
        self.browser_sessions.push(session.clone());
        Ok(json!({"sessionId":session}))
    }

    /** Create a flattened session for a tab and announce it on the root (auto-attach). */
    fn attach_tab(&mut self, tab: &str, info: Value) -> String {
        self.attach_tab_for(tab, info, None)
    }

    /** Create a flattened session for a tab and announce it to `asker` (None: the root). */
    fn attach_tab_for(&mut self, tab: &str, mut info: Value, asker: Option<String>) -> String {
        self.adopt(tab);
        self.next_session += 1;
        let session = format!(
            "{}{:032X}",
            tab.replace('-', "").to_ascii_uppercase(),
            self.next_session
        );
        self.sessions.insert(session.clone(), tab.to_owned());
        info["attached"] = json!(true);
        let mut event = json!({"method":"Target.attachedToTarget","params":{"sessionId":session,"targetInfo":info,"waitingForDebugger":self.wait_for_debugger}});
        if let Some(asker) = asker {
            event["sessionId"] = json!(asker);
        }
        self.events.push(event);
        session
    }

    /** Target.detachFromTarget. */
    fn detach(&mut self, _: Option<&str>, params: &Value) -> Result<Value, Fault> {
        if let Some(session) = params["sessionId"].as_str() {
            self.sessions.remove(session);
            self.emit(
                None,
                "Target.detachedFromTarget",
                json!({"sessionId":session}),
            );
        }
        Ok(json!({}))
    }

    /** Target.createTarget opens an agent tab of this lane's own. */
    fn create_target(&mut self, _: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let url = params["url"]
            .as_str()
            .filter(|url| !url.is_empty() && *url != "about:blank");
        let opened = self.call(
            "tab.open",
            url.map_or_else(|| json!({}), |url| json!({"url":url})),
        )?;
        if let Some(error) = opened.get("navigation_error") {
            return Err(Fault::native(error));
        }
        let tab = opened["tab_id"].as_str().unwrap_or_default().to_owned();
        self.targets.insert(
            tab.clone(),
            super::Target {
                epoch: opened["control_epoch"].as_u64(),
                owned: true,
                ..super::Target::default()
            },
        );
        self.adopt(&tab);
        let info = json!({"targetId":tab,"type":"page","title":opened["page"]["title"].as_str().unwrap_or("New Tab"),"url":opened["page"]["url"].as_str().unwrap_or("about:blank"),"attached":false,"canAccessOpener":false,"browserContextId":"default"});
        self.emit(None, "Target.targetCreated", json!({"targetInfo":info}));
        if self.auto_attach {
            self.attach_tab(&tab, info);
        }
        Ok(json!({"targetId":tab}))
    }

    /** Target.closeTarget closes a tab this lane opened. */
    fn close_target(&mut self, _: Option<&str>, params: &Value) -> Result<Value, Fault> {
        let tab = params["targetId"]
            .as_str()
            .ok_or_else(|| Fault::invalid("targetId is required."))?
            .to_owned();
        self.close_tab(&tab)?;
        Ok(json!({"success":true}))
    }

    /** Close a tab and forget its sessions; Page.close shares this. */
    pub(super) fn close_tab(&mut self, tab: &str) -> Result<(), Fault> {
        let epoch = self.epoch(tab);
        self.call("tab.close", json!({"tab_id":tab,"control_epoch":epoch}))?;
        self.targets.remove(tab);
        let gone: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, attached)| attached.as_str() == tab)
            .map(|(session, _)| session.clone())
            .collect();
        for session in gone {
            self.sessions.remove(&session);
            self.emit(
                None,
                "Target.detachedFromTarget",
                json!({"sessionId":session,"targetId":tab}),
            );
        }
        self.emit(None, "Target.targetDestroyed", json!({"targetId":tab}));
        Ok(())
    }
}
