/** Actionable failures cross service boundaries without stack traces or retries. */
use serde::{Deserialize, Serialize};

/// All native operations share typed errors.
pub type EngineResult<T> = Result<T, EngineError>;
/// Script coordinates use one-based lines/UTF-16 columns and a zero-based byte offset.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceLocation {
    /// Start of the token in the original UTF-8 source, not a source excerpt.
    pub byte_offset: usize,
    /// CRLF counts once; CR, LF, LS and PS advance the line.
    pub line: usize,
    /// Astral characters advance this column by two code units.
    pub column: usize,
    /// One-based page script index; absent for standalone V9 calls and handlers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script_index: Option<usize>,
}
/// A compact typed phase avoids allocating strings in every engine error.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptPhase {
    Scan,
    Parse,
    Execute,
    Dom,
}
impl From<&str> for ScriptPhase {
    fn from(value: &str) -> Self {
        match value {
            "scan" => Self::Scan,
            "parse" => Self::Parse,
            "execute" => Self::Execute,
            "dom" => Self::Dom,
            _ => unreachable!("phase is a static VM choice"),
        }
    }
}
impl std::ops::Deref for ScriptPhase {
    type Target = str;
    fn deref(&self) -> &str {
        match self {
            Self::Scan => "scan",
            Self::Parse => "parse",
            Self::Execute => "execute",
            Self::Dom => "dom",
        }
    }
}
/// Handler and script locations refer to distinct source texts.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptInput {
    Script,
    Handler,
}
impl From<&str> for ScriptInput {
    fn from(value: &str) -> Self {
        match value {
            "script" => Self::Script,
            "handler" => Self::Handler,
            _ => unreachable!("input kind is a static VM choice"),
        }
    }
}
impl std::ops::Deref for ScriptInput {
    type Target = str;
    fn deref(&self) -> &str {
        match self {
            Self::Script => "script",
            Self::Handler => "handler",
        }
    }
}

/// A failure tells an agent what happened and how to recover.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EngineError {
    /// Stable machine-readable classification.
    pub code: String,
    /// Short explanation without secrets or process internals.
    pub message: String,
    /// Mutations are never retried automatically.
    pub retryable: bool,
    /// One concrete next action.
    pub hint: String,
    /// Navigation failures disclose whether the old document is still committed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub previous_page_retained: bool,
    /// Identifies an engine when the session must be restarted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    /// Constraint errors identify a field without serializing its value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalid_node: Option<Box<usize>>,
    /// Located script diagnostics never require parsing human-facing text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Box<SourceLocation>>,
    /// Scan/parse/execute/dom separates unsupported syntax from failed execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<ScriptPhase>,
    /// Handler coordinates refer to the handler text, not the complete page script.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_kind: Option<ScriptInput>,
}

impl EngineError {
    /// Centralizes the wire shape and disables implicit retry.
    pub fn new(code: &str, message: impl Into<String>, hint: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
            hint: hint.into(),
            previous_page_retained: false,
            service: None,
            invalid_node: None,
            location: None,
            phase: None,
            input_kind: None,
        }
    }
    /// Invalid input can be corrected without restarting a healthy session.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(
            "invalid_request",
            message,
            "Use runtime.capabilities and the documented request schema.",
        )
    }
    /// Unsupported syntax never silently becomes inert content.
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(
            "unsupported_feature",
            message,
            "Use the supported subset documented in mars/README.md.",
        )
    }
    /// Allocation and work budgets fail explicitly instead of truncating output.
    pub fn limit(name: &str, bound: usize) -> Self {
        Self::new(
            "limit_exceeded",
            format!("{name} exceeds the limit of {bound}."),
            "Reduce the input and retry.",
        )
    }
    /// An exhausted shared navigation budget can be retried without replaying a mutation.
    pub fn navigation_timeout() -> Self {
        let mut error = Self::new(
            "timeout",
            "The page and its resources exceeded the navigation deadline.",
            "Retry the page when the destination is available.",
        );
        error.retryable = true;
        error
    }
    /// Transport faults make a private service and its runtime unusable.
    pub fn service(name: &str, timeout: bool) -> Self {
        let mut error = Self::new(
            if timeout {
                "timeout"
            } else {
                "service_unavailable"
            },
            format!("The {name} service is unavailable."),
            "Restart the runtime; no operation was replayed.",
        );
        error.service = Some(name.into());
        error
    }
}
