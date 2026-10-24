/** page.act runs an ordered batch of native steps under one admission, one revision at a time. */
use super::observe::Query;
use super::supervisor::Runtime;
use super::{authority, supervisor::tab_key};
use crate::protocol::{EngineError, EngineResult, Request, parameters};
use serde::Deserialize;
use serde_json::{Value, json};

/** Steps in one batch; longer flows are split so a person can intervene between them. */
pub(super) const ACT_STEPS: usize = 16;

/** A step targets an exact node_id or a find query that must match exactly one node. */
struct Target {
    node_id: Option<String>,
    find: Option<Query>,
}

/** Each step mirrors one existing native operation; nothing here bypasses its checks. */
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Step {
    Type {
        #[serde(default)]
        node_id: Option<String>,
        #[serde(default)]
        find: Option<Query>,
        text: String,
    },
    Click {
        #[serde(default)]
        node_id: Option<String>,
        #[serde(default)]
        find: Option<Query>,
    },
    SetChecked {
        #[serde(default)]
        node_id: Option<String>,
        #[serde(default)]
        find: Option<Query>,
        checked: bool,
    },
    Select {
        #[serde(default)]
        node_id: Option<String>,
        #[serde(default)]
        find: Option<Query>,
        #[serde(default)]
        option_id: Option<String>,
        #[serde(default)]
        option: Option<String>,
    },
    Submit {
        #[serde(default)]
        node_id: Option<String>,
        #[serde(default)]
        find: Option<Query>,
    },
    Scroll {
        y: f64,
    },
    Navigate {
        url: String,
    },
}

/** The batch names the page and revision it was planned against. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Act {
    page_id: String,
    expected_revision: u64,
    steps: Vec<Value>,
    #[serde(default)]
    result: Option<String>,
}

impl Step {
    /** The public action name reported back per step. */
    fn name(&self) -> &'static str {
        match self {
            Self::Type { .. } => "type",
            Self::Click { .. } => "click",
            Self::SetChecked { .. } => "set_checked",
            Self::Select { .. } => "select",
            Self::Submit { .. } => "submit",
            Self::Scroll { .. } => "scroll",
            Self::Navigate { .. } => "navigate",
        }
    }
    /** The node target, when the step has one. */
    fn target(&self) -> Option<Target> {
        let (node_id, find) = match self {
            Self::Type { node_id, find, .. }
            | Self::Click { node_id, find }
            | Self::SetChecked { node_id, find, .. }
            | Self::Select { node_id, find, .. }
            | Self::Submit { node_id, find } => (node_id, find),
            Self::Scroll { .. } | Self::Navigate { .. } => return None,
        };
        Some(Target {
            node_id: node_id.clone(),
            find: find.clone(),
        })
    }
}

impl Runtime {
    /** Revalidate the admission between steps so a human takeover stops the batch at once. */
    fn still_permitted(&self) -> EngineResult<()> {
        let Some(authority) = &self.authority else {
            return Ok(());
        };
        let permit = self.permit.ok_or_else(authority::denied)?;
        authority::lock(authority)?.validate_tab(permit, tab_key(&self.active_tab_id)?)
    }
    /** Build the native request for one step against the current page and revision. */
    fn step_request(&self, step: &Step) -> EngineResult<Request> {
        let page = self.current()?;
        let mut params = json!({"page_id":page.id(),"expected_revision":page.revision});
        if let Some(target) = step.target() {
            let node_id = match (target.node_id, target.find) {
                (Some(id), None) => id,
                (None, Some(query)) => page.resolve(&query)?,
                _ => {
                    return Err(EngineError::invalid(
                        "A step targets exactly one of node_id or find.",
                    ));
                }
            };
            params["node_id"] = json!(node_id);
        }
        let op = match step {
            Step::Type { text, .. } => {
                params["text"] = json!(text);
                "page.type"
            }
            Step::Click { .. } => "page.click",
            Step::SetChecked { checked, .. } => {
                params["checked"] = json!(checked);
                "page.set_checked"
            }
            Step::Select {
                option_id, option, ..
            } => {
                let option_id = match (option_id, option) {
                    (Some(id), None) => id.clone(),
                    (None, Some(text)) => {
                        page.option_by_text(params["node_id"].as_str().unwrap_or_default(), text)?
                    }
                    _ => {
                        return Err(EngineError::invalid(
                            "A select step names exactly one of option_id or option.",
                        ));
                    }
                };
                params["option_id"] = json!(option_id);
                "page.select"
            }
            Step::Submit { .. } => "page.submit",
            Step::Scroll { y } => {
                params["y"] = json!(y);
                "page.scroll"
            }
            Step::Navigate { url } => {
                params = json!({"url":url});
                "page.navigate"
            }
        };
        Ok(Request::new("act", op, params))
    }
    /** Run one step through the ordinary native dispatcher and its checks. */
    fn run_step(&mut self, raw: Value) -> EngineResult<(&'static str, Value)> {
        let step: Step = parameters(raw)?;
        let name = step.name();
        self.still_permitted()?;
        let request = self.step_request(&step)?;
        self.dispatch_active(request)?;
        let page = self.current()?;
        Ok((name, json!({"page_id":page.id(),"revision":page.revision})))
    }
    /**
     * page.act: each step re-validates against the revision the previous step produced, so the
     * agent plans once and the runtime never acts on a page it did not just observe. The first
     * failure stops the batch; committed earlier steps stay committed and are reported.
     */
    pub(super) fn act(&mut self, params: Value) -> EngineResult<Value> {
        let input: Act = parameters(params)?;
        if input.steps.is_empty() || input.steps.len() > ACT_STEPS {
            return Err(EngineError::limit("act steps", ACT_STEPS));
        }
        let mode = input.result.as_deref().unwrap_or("summary");
        if !["summary", "diff", "none"].contains(&mode) {
            return Err(EngineError::invalid(
                "result must be summary, diff or none.",
            ));
        }
        let page = self.current()?;
        if page.id() != input.page_id || page.revision != input.expected_revision {
            return Err(EngineError::new(
                "invalid_target",
                "The batch targets a stale page or revision.",
                "Call page_summary and plan the batch against the current revision.",
            ));
        }
        let before = page.snapshot();
        let total = input.steps.len();
        let mut results = Vec::new();
        let mut failed = false;
        for (index, raw) in input.steps.into_iter().enumerate() {
            match self.run_step(raw) {
                Ok((name, mut value)) => {
                    value["index"] = json!(index);
                    value["action"] = json!(name);
                    value["ok"] = json!(true);
                    results.push(value);
                }
                Err(error) => {
                    results.push(json!({"index":index,"ok":false,"error":error}));
                    failed = true;
                    break;
                }
            }
        }
        let page = self.current()?;
        let mut value = json!({"ok":!failed,"completed":results.iter().filter(|step|step["ok"]==true).count(),"total":total,"steps":results,"page_id":page.id(),"revision":page.revision});
        match mode {
            "diff" if page.id() == before.page_id => {
                value["diff"] = super::observe::diff(page, &before, &page.snapshot());
            }
            "none" => {}
            _ => value["summary"] = page.summary(),
        }
        Ok(value)
    }
}
