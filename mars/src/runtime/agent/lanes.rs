/** Agent lanes: each connected agent opens, drives and closes only its own tabs. */
use super::super::authority::{self, DEFAULT_LANE, Permit};
use super::super::supervisor::{Runtime, tab_key};
use crate::constants::ID_BYTES;
use crate::protocol::{EngineError, EngineResult, Request, parameters};
use serde_json::{Value, json};

/// Tabs one lane may hold open at once, so a looping agent cannot fill the strip.
const LANE_TABS: usize = 8;

/** An agent tab may start on a page; without one it stays blank. */
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Open {
    #[serde(default)]
    url: Option<String>,
}

/** Closing names the tab and the epoch the agent observed when it opened it. */
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Close {
    tab_id: String,
    control_epoch: u64,
}

/**
 * The front door, never the agent, names the lane: connections overwrite any `lane` a client
 * sends, and requests without one belong to the single stdio agent.
 */
pub(super) fn take_lane(params: &mut Value) -> EngineResult<String> {
    let lane = params
        .as_object_mut()
        .and_then(|params| params.remove("lane"));
    match lane {
        None => Ok(DEFAULT_LANE.into()),
        Some(Value::String(lane)) if !lane.is_empty() && lane.len() <= ID_BYTES => Ok(lane),
        Some(_) => Err(EngineError::invalid("Invalid agent lane.")),
    }
}

/** Capabilities name the agent tab operations next to the human-only ones. */
pub(super) fn describe(result: &mut Value) {
    result["operations"]["tab.open"] = json!({"url":"optional string","agent":"opens a background tab under the caller's lane","human":"opens and selects a tab"});
    result["operations"]["tab.close"] =
        json!({"tab_id":"string","control_epoch":"u64 (agents: only tabs their lane opened)"});
    result["lane_tabs"] = json!(LANE_TABS);
    for (operation, shape) in [
        ("page.screenshot", json!({"node_id":"optional string"})),
        (
            "page.click_at",
            json!({"page_id":"string","expected_revision":"u64","x":"viewport points","y":"viewport points"}),
        ),
        (
            "page.hover",
            json!({"x":"viewport points","y":"viewport points"}),
        ),
        (
            "page.scroll_at",
            json!({"page_id":"string","expected_revision":"u64","x":"viewport points","y":"viewport points","delta_y":"points"}),
        ),
        (
            "page.press",
            json!({"page_id":"string","expected_revision":"u64","node_id":"optional string (required for editing keys)","key":"optional: Enter, Space, Backspace, PageDown, PageUp, Home, End or one character","text":"optional: characters appended to the field","modifiers":"optional [Shift|Control|Alt|Meta]"}),
        ),
        ("page.console", json!({})),
        ("page.network", json!({})),
    ] {
        result["operations"][operation] = shape;
        result["operations"][operation]["tab_id"] = json!("string");
    }
}

impl Runtime {
    /** Agent tab creation and closure, with the lane rules Oya's desktop lanes follow. */
    pub(super) fn agent_tab(
        &mut self,
        request: Request,
        permit: Permit,
        lane: &str,
    ) -> EngineResult<Value> {
        if self.authority.is_none() {
            return Err(EngineError::unsupported(
                "Headless agent sessions have one implicit tab and no lanes.",
            ));
        }
        if request.op == "tab.open" {
            let input: Open = parameters(request.params)?;
            return self.open_agent_tab(lane, input.url, &request.id);
        }
        let input: Close = parameters(request.params)?;
        let key = tab_key(&input.tab_id)?;
        {
            let state = self.lane_authority()?;
            state.validate_tab(permit, key)?;
            if input.control_epoch != permit.epoch || !state.opened_by(key, lane) {
                return Err(authority::denied());
            }
        }
        let mut tabs = self.close_tab(&input.tab_id)?;
        tabs["closed"] = json!(input.tab_id);
        Ok(tabs)
    }

    /** A fresh background tab: the person's selection and tabs stay as they were. */
    fn open_agent_tab(
        &mut self,
        lane: &str,
        url: Option<String>,
        request_id: &str,
    ) -> EngineResult<Value> {
        if self.lane_authority()?.opened_count(lane) >= LANE_TABS {
            return Err(EngineError::limit("agent tabs per lane", LANE_TABS));
        }
        let selected = self.selected_tab_id().to_owned();
        self.open_tab()?;
        let id = self.selected_tab_id().to_owned();
        self.select_tab_by_id(&selected)?;
        let key = tab_key(&id)?;
        let (_, epoch) = self.lane_authority()?.open_for_lane(key, lane)?;
        let mut result = json!({"tab_id":id,"owner":"agent","control_epoch":epoch,"lane":lane});
        if let Some(url) = url {
            let permit = Permit {
                owner: authority::Owner::Agent,
                epoch,
                tab_id: Some(key),
            };
            self.permit = Some(permit);
            let request = Request::new(request_id, "page.navigate", json!({"url":url,"tab_id":id}));
            match self.handle(request) {
                Ok(page) => result["page"] = page,
                Err(error) => result["navigation_error"] = json!(error),
            }
        }
        Ok(result)
    }

    /** Lanes exist only where a person shares the browser with agents. */
    fn lane_authority(&self) -> EngineResult<std::sync::MutexGuard<'_, authority::Authority>> {
        authority::lock(self.authority.as_ref().ok_or_else(authority::denied)?)
    }
}
