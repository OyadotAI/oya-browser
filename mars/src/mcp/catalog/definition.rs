/** Only this reviewed tool table can select native operations from the MCP front door. */
use super::field::Field;
use crate::protocol::{EngineError, EngineResult};
use serde_json::{Value, json};
/** One entry owns the public name, native operation, discovery copy and complete required argument set. */
pub(super) struct Definition {
    /// Discovery names are exact lookup keys rather than transformed arbitrary native operations.
    pub name: &'static str,
    /// The selected native operation still performs its normal authority and target validation.
    pub operation: &'static str,
    /// Designer wording describes native behavior without asserting successful execution.
    description: &'static str,
    /// Every listed field is required; an empty list accepts only an empty object.
    fields: &'static [Field],
    /// Fields a caller may add; anything not listed in either set is refused.
    optional: &'static [Field],
}
impl Definition {
    /** Object schemas reject extra fields and deliberately omit an output schema for this initial subset. */
    pub fn schema(&self) -> Value {
        let properties = self
            .fields
            .iter()
            .chain(self.optional)
            .map(|field| (field.name().to_owned(), field.schema()))
            .collect::<serde_json::Map<_, _>>();
        let required = self
            .fields
            .iter()
            .map(|field| field.name())
            .collect::<Vec<_>>();
        json!({"name":self.name,"description":self.description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false}})
    }
    /** Semantic validation enforces exact keys and UTF-8 limits even for callers that bypass JSON Schema. */
    pub fn validate(&self, arguments: &Value) -> EngineResult<()> {
        let Some(arguments) = arguments.as_object() else {
            return Err(invalid());
        };
        let optional = self
            .optional
            .iter()
            .filter(|field| arguments.contains_key(field.name()))
            .collect::<Vec<_>>();
        if arguments.len() != self.fields.len() + optional.len()
            || self
                .fields
                .iter()
                .chain(optional)
                .any(|field| !field.valid(arguments.get(field.name())))
        {
            return Err(invalid());
        }
        Ok(())
    }
}
/** Invalid arguments reveal no supplied keys, identifiers, text, URL or field values. */
fn invalid() -> EngineError {
    EngineError::invalid("Tool arguments do not match the documented native tool schema.")
}
use Field::{
    Absent, AfterRevision, Append, Checked, Compact, Contains, DeltaY, Epoch, Fields, Key, Label,
    Limit, Modifiers, Name, NavigatedFrom, Node, Option, Page, PointX, PointY, ResultMode,
    Revision, Role, Since, Steps, Tab, Text, Timeout, Url, UrlPart, VisibleOnly, Y,
};
/** The exhaustive MCP exposure deliberately excludes native human-only and unreviewed operations. */
pub(super) const TOOLS: &[Definition] = &[
    Definition {
        name: "runtime_capabilities",
        operation: "runtime.capabilities",
        description: "List this native runtime's supported features and limits.",
        fields: &[],
        optional: &[],
    },
    Definition {
        name: "tab_list",
        operation: "tab.list",
        description: "List native tabs and their identities.",
        fields: &[],
        optional: &[],
    },
    Definition {
        name: "control_status",
        operation: "control.status",
        description: "Read ownership and the current control epoch for one native tab.",
        fields: &[Tab],
        optional: &[],
    },
    Definition {
        name: "page_inspect",
        operation: "page.inspect",
        description: "Inspect one native tab's committed page, controls and current identities.",
        fields: &[Tab],
        optional: &[Fields, VisibleOnly, Compact],
    },
    Definition {
        name: "page_summary",
        operation: "page.summary",
        description: "Read a short outline of one tab's page: title, headings, landmarks and numbered controls with their current values.",
        fields: &[Tab],
        optional: &[],
    },
    Definition {
        name: "page_find",
        operation: "page.find",
        description: "Find nodes by role, accessible name, label or text and return their node IDs and bounds.",
        fields: &[Tab],
        optional: &[Role, Name, Label, Contains, Limit, VisibleOnly],
    },
    Definition {
        name: "page_diff",
        operation: "page.diff",
        description: "List the nodes added, removed or changed on the current page since an earlier revision.",
        fields: &[Tab, Since],
        optional: &[],
    },
    Definition {
        name: "page_wait",
        operation: "page.wait",
        description: "Wait until one condition holds on the tab's page, or until the timeout passes.",
        fields: &[Tab],
        optional: &[
            Contains,
            Role,
            Name,
            Label,
            Absent,
            UrlPart,
            AfterRevision,
            NavigatedFrom,
            Timeout,
        ],
    },
    Definition {
        name: "page_act",
        operation: "page.act",
        description: "Run several steps in order on one tab, each against the revision the last step produced. Stops at the first failure.",
        fields: &[Tab, Epoch, Page, Revision, Steps],
        optional: &[ResultMode],
    },
    Definition {
        name: "page_layout",
        operation: "page.layout",
        description: "Read the native display geometry for one tab in logical points.",
        fields: &[Tab],
        optional: &[],
    },
    Definition {
        name: "page_navigate",
        operation: "page.navigate",
        description: "Load a supported URL in the specified native tab.",
        fields: &[Tab, Epoch, Url],
        optional: &[],
    },
    Definition {
        name: "page_type",
        operation: "page.type",
        description: "Replace an enabled editable field's value with literal text.",
        fields: &[Tab, Epoch, Page, Node, Revision, Text],
        optional: &[],
    },
    Definition {
        name: "page_click",
        operation: "page.click",
        description: "Activate a supported native link or control by its discovered node identity.",
        fields: &[Tab, Epoch, Page, Node, Revision],
        optional: &[],
    },
    Definition {
        name: "page_set_checked",
        operation: "page.set_checked",
        description: "Set a supported checkbox or radio control's checked state explicitly.",
        fields: &[Tab, Epoch, Page, Node, Revision, Checked],
        optional: &[],
    },
    Definition {
        name: "page_select",
        operation: "page.select",
        description: "Select an enabled option using its discovered native option identity.",
        fields: &[Tab, Epoch, Page, Node, Revision, Option],
        optional: &[],
    },
    Definition {
        name: "page_submit",
        operation: "page.submit",
        description: "Submit the targeted form using the native validation and submitter rules.",
        fields: &[Tab, Epoch, Page, Node, Revision],
        optional: &[],
    },
    Definition {
        name: "page_back",
        operation: "page.back",
        description: "Go back in the specified tab, respecting native form-resubmission safeguards.",
        fields: &[Tab, Epoch],
        optional: &[],
    },
    Definition {
        name: "page_forward",
        operation: "page.forward",
        description: "Go forward in the specified tab, respecting native form-resubmission safeguards.",
        fields: &[Tab, Epoch],
        optional: &[],
    },
    Definition {
        name: "page_reload",
        operation: "page.reload",
        description: "Reload the specified tab when its current page can be replayed safely.",
        fields: &[Tab, Epoch],
        optional: &[],
    },
    Definition {
        name: "page_scroll",
        operation: "page.scroll",
        description: "Set the document's vertical scroll offset in logical points.",
        fields: &[Tab, Epoch, Page, Revision, Y],
        optional: &[],
    },
    Definition {
        name: "tab_open",
        operation: "tab.open",
        description: "Open a background tab of your own, under your control, optionally on a URL. Returns its tab_id and control_epoch. The person's tabs are untouched.",
        fields: &[],
        optional: &[Url],
    },
    Definition {
        name: "tab_close",
        operation: "tab.close",
        description: "Close a tab you opened with tab_open. Tabs you did not open are refused.",
        fields: &[Tab, Epoch],
        optional: &[],
    },
    Definition {
        name: "page_screenshot",
        operation: "page.screenshot",
        description: "Capture the tab's visible viewport, or one element's box, as a PNG image.",
        fields: &[Tab],
        optional: &[Node],
    },
    Definition {
        name: "page_click_at",
        operation: "page.click_at",
        description: "Click whatever a person would click at a viewport point. Reports the node that was hit.",
        fields: &[Tab, Epoch, Page, Revision, PointX, PointY],
        optional: &[],
    },
    Definition {
        name: "page_hover",
        operation: "page.hover",
        description: "Read the node under a viewport point and what a click there would reach. Changes nothing.",
        fields: &[Tab, PointX, PointY],
        optional: &[],
    },
    Definition {
        name: "page_scroll_at",
        operation: "page.scroll_at",
        description: "Turn the wheel at a viewport point, scrolling the page by delta_y points.",
        fields: &[Tab, Epoch, Page, Revision, PointX, PointY, DeltaY],
        optional: &[],
    },
    Definition {
        name: "page_press",
        operation: "page.press",
        description: "Press a key on a field or control (Enter submits, Space toggles, Backspace erases), scroll with PageDown/PageUp/Home/End, or append text. Give exactly one of key or text.",
        fields: &[Tab, Epoch, Page, Revision],
        optional: &[Node, Key, Append, Modifiers],
    },
    Definition {
        name: "page_console",
        operation: "page.console",
        description: "Read the engine's recent messages about this page: skipped scripts and refused resources.",
        fields: &[Tab],
        optional: &[],
    },
    Definition {
        name: "page_network",
        operation: "page.network",
        description: "List the loads behind the current page: the document with method and status, and images with type and size. No headers or cookies.",
        fields: &[Tab],
        optional: &[],
    },
    Definition {
        name: "control_release",
        operation: "control.release",
        description: "Return agent control of the specified native tab to the person.",
        fields: &[Tab, Epoch],
        optional: &[],
    },
];
