/** Tool schemas and semantic checks share exact native types, copy and UTF-8 byte bounds. */
use crate::constants::{ID_BYTES, TEXT_BYTES, URL_BYTES};
use crate::runtime::WAIT_MAX_MS;
use serde_json::{Value, json};
/** Reused target fields retain one spelling and description across all native tools. */
#[derive(Clone, Copy)]
pub(super) enum Field {
    Tab,
    Epoch,
    Page,
    Node,
    Revision,
    Option,
    Text,
    Checked,
    Url,
    Y,
    Fields,
    VisibleOnly,
    Compact,
    Role,
    Name,
    Label,
    Contains,
    Limit,
    Since,
    Steps,
    ResultMode,
    Absent,
    UrlPart,
    AfterRevision,
    NavigatedFrom,
    Timeout,
    PointX,
    PointY,
    DeltaY,
    Key,
    Append,
    Modifiers,
}
/** Batches stay small enough that a person can step in between them. */
const STEPS: usize = 16;
/** Projected inspect fields one call may name. */
const FIELDS: usize = 32;
/** page_find never returns more than this many matches. */
const MATCHES: usize = 50;
/** A key name is short; the longest supported one is PageDown. */
const KEY_BYTES: usize = 16;
/** Shift, Control, Alt and Meta, each at most once. */
const MODIFIERS: usize = 4;
impl Field {
    /** Native parameter names are fixed identifiers, never derived from external operation strings. */
    pub fn name(self) -> &'static str {
        match self {
            Self::Tab => "tab_id",
            Self::Epoch => "control_epoch",
            Self::Page => "page_id",
            Self::Node => "node_id",
            Self::Revision => "expected_revision",
            Self::Option => "option_id",
            Self::Text => "text",
            Self::Checked => "checked",
            Self::Url | Self::UrlPart => "url",
            Self::Y => "y",
            Self::Fields => "fields",
            Self::VisibleOnly => "visible_only",
            Self::Compact => "compact",
            Self::Role => "role",
            Self::Name => "name",
            Self::Label => "label",
            Self::Contains => "text",
            Self::Limit => "limit",
            Self::Since => "since_revision",
            Self::Steps => "steps",
            Self::ResultMode => "result",
            Self::Absent => "absent",
            Self::AfterRevision => "after_revision",
            Self::NavigatedFrom => "navigated_from",
            Self::Timeout => "timeout_ms",
            Self::PointX => "x",
            Self::PointY => "y",
            Self::DeltaY => "delta_y",
            Self::Key => "key",
            Self::Append => "text",
            Self::Modifiers => "modifiers",
        }
    }
    /** Exact designer guidance makes discovered preconditions meaningful to connecting agents. */
    fn description(self) -> &'static str {
        match self {
            Self::Tab => {
                "Exact native tab ID returned by tab_list. No tab is selected automatically."
            }
            Self::Epoch => {
                "The epoch returned by control_status for this tab after the human grant. A changed epoch is refused."
            }
            Self::Page => "Current page ID returned by page_inspect for this tab.",
            Self::Node => "Native node ID returned by page_inspect for this page.",
            Self::Revision => "Observed page revision. A newer committed change causes refusal.",
            Self::Option => {
                "Native option ID discovered for the targeted select; not its author value or label."
            }
            Self::Y => {
                "Desired document scroll offset in logical points. The native runtime clamps it to the page."
            }
            Self::Text => "Literal replacement text.",
            Self::Checked => "Boolean checked state.",
            Self::Url => "Supported URL to load in the specified native tab.",
            Self::Fields => {
                "Node fields to keep, for example [\"role\",\"name\",\"value\"]. node_id is always kept."
            }
            Self::VisibleOnly => "Keep only nodes inside the current viewport.",
            Self::Compact => "Omit fields that hold their default value.",
            Self::Role => {
                "Exact role as page_inspect reports it, such as button, link, textbox or checkbox."
            }
            Self::Name => "Accessible name equal to this, ignoring case and surrounding spaces.",
            Self::Label => "Accessible name or placeholder containing this, ignoring case.",
            Self::Contains => "Text the node or page contains, ignoring case.",
            Self::Limit => "Most matches to return, 1 to 50. Defaults to 10.",
            Self::Since => "A revision of the current page read earlier from this tab.",
            Self::Steps => {
                "Ordered steps, each {\"action\": type|click|set_checked|select|submit|scroll|navigate, plus node_id or find {role,name,label,text}, and text, checked, option_id or option, y or url}. At most 16; the batch stops at the first failure."
            }
            Self::ResultMode => {
                "What to return after the batch: summary (default), diff since the starting revision, or none."
            }
            Self::Absent => "Wait until no node matches role, name or label.",
            Self::UrlPart => "Wait until the page URL contains this.",
            Self::AfterRevision => "Wait until the page revision is greater than this.",
            Self::NavigatedFrom => "Wait until the tab shows a page other than this page ID.",
            Self::Timeout => "Longest wait in milliseconds, at most 20000. Defaults to 5000.",
            Self::PointX => "Viewport x in logical points, as page_screenshot shows it.",
            Self::PointY => {
                "Viewport y in logical points: page_find bounds y minus the scroll offset."
            }
            Self::DeltaY => "Points to scroll; positive scrolls down.",
            Self::Key => "Enter, Space, Backspace, PageDown, PageUp, Home, End or one character.",
            Self::Append => "Characters to append to the field's current value.",
            Self::Modifiers => {
                "Held modifiers: Shift, Control, Alt, Meta. Control or Meta with Backspace clears the field; Alt erases a word."
            }
        }
    }
    /** Discovery length limits supplement, rather than replace, the mandatory byte checks at translation. */
    pub fn schema(self) -> Value {
        let mut schema = match self {
            Self::Tab | Self::Page | Self::Node | Self::Option => {
                json!({"type":"string","minLength":1,"maxLength":ID_BYTES})
            }
            Self::Text => json!({"type":"string","maxLength":TEXT_BYTES}),
            Self::Url => json!({"type":"string","maxLength":URL_BYTES}),
            Self::Epoch | Self::Revision => {
                json!({"type":"integer","minimum":0,"maximum":u64::MAX})
            }
            Self::Checked => json!({"type":"boolean"}),
            Self::Y => json!({"type":"number"}),
            Self::Fields => {
                json!({"type":"array","items":{"type":"string","maxLength":ID_BYTES},"maxItems":FIELDS})
            }
            Self::VisibleOnly | Self::Compact | Self::Absent => json!({"type":"boolean"}),
            Self::Role | Self::Name | Self::Label | Self::Contains | Self::NavigatedFrom => {
                json!({"type":"string","minLength":1,"maxLength":TEXT_BYTES})
            }
            Self::UrlPart => json!({"type":"string","minLength":1,"maxLength":URL_BYTES}),
            Self::Limit => json!({"type":"integer","minimum":1,"maximum":MATCHES}),
            Self::Since | Self::AfterRevision => {
                json!({"type":"integer","minimum":0,"maximum":u64::MAX})
            }
            Self::Timeout => json!({"type":"integer","minimum":0,"maximum":WAIT_MAX_MS}),
            Self::PointX | Self::PointY | Self::DeltaY => json!({"type":"number"}),
            Self::Key => json!({"type":"string","minLength":1,"maxLength":KEY_BYTES}),
            Self::Append => json!({"type":"string","minLength":1,"maxLength":TEXT_BYTES}),
            Self::Modifiers => {
                json!({"type":"array","items":{"type":"string","enum":["Shift","Control","Alt","Meta"]},"maxItems":MODIFIERS})
            }
            Self::Steps => {
                json!({"type":"array","minItems":1,"maxItems":STEPS,"items":{"type":"object","required":["action"],"properties":{"action":{"type":"string","enum":["type","click","set_checked","select","submit","scroll","navigate"]}}}})
            }
            Self::ResultMode => json!({"type":"string","enum":["summary","diff","none"]}),
        };
        schema["description"] = json!(self.description());
        schema
    }
    /** Present values retain their native JSON type without coercion or truncation. */
    pub fn valid(self, value: Option<&Value>) -> bool {
        let Some(value) = value else {
            return false;
        };
        let text = |limit: usize| {
            value
                .as_str()
                .is_some_and(|value| !value.is_empty() && value.len() <= limit)
        };
        match self {
            Self::Tab | Self::Page | Self::Node | Self::Option => value
                .as_str()
                .is_some_and(|value| !value.is_empty() && value.len() <= ID_BYTES),
            Self::Text => value
                .as_str()
                .is_some_and(|value| value.len() <= TEXT_BYTES),
            Self::Url => value.as_str().is_some_and(|value| value.len() <= URL_BYTES),
            Self::Epoch | Self::Revision => value.as_u64().is_some(),
            Self::Checked => value.as_bool().is_some(),
            Self::Y => value.as_f64().is_some_and(f64::is_finite),
            Self::Fields => value.as_array().is_some_and(|fields| {
                fields.len() <= FIELDS
                    && fields
                        .iter()
                        .all(|field| field.as_str().is_some_and(|field| field.len() <= ID_BYTES))
            }),
            Self::VisibleOnly | Self::Compact | Self::Absent => value.is_boolean(),
            Self::Role | Self::Name | Self::Label | Self::Contains | Self::NavigatedFrom => {
                text(TEXT_BYTES)
            }
            Self::UrlPart => text(URL_BYTES),
            Self::Limit => value
                .as_u64()
                .is_some_and(|limit| (1..=MATCHES as u64).contains(&limit)),
            Self::Since | Self::AfterRevision => value.as_u64().is_some(),
            Self::Timeout => value.as_u64().is_some_and(|timeout| timeout <= WAIT_MAX_MS),
            Self::PointX | Self::PointY | Self::DeltaY => {
                value.as_f64().is_some_and(f64::is_finite)
            }
            Self::Key => text(KEY_BYTES),
            Self::Append => text(TEXT_BYTES),
            Self::Modifiers => value.as_array().is_some_and(|modifiers| {
                modifiers.len() <= MODIFIERS && modifiers.iter().all(Value::is_string)
            }),
            Self::Steps => value.as_array().is_some_and(|steps| {
                (1..=STEPS).contains(&steps.len())
                    && steps
                        .iter()
                        .all(|step| step.get("action").is_some_and(Value::is_string))
            }),
            Self::ResultMode => value
                .as_str()
                .is_some_and(|mode| ["summary", "diff", "none"].contains(&mode)),
        }
    }
}
