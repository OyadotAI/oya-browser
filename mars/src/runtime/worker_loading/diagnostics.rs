/** Fixed worker diagnostic categories keep opt-in traces useful without storing page secrets. */
use crate::protocol::EngineError;
use crate::runtime::supervisor::perf;
use std::time::Instant;

/** Source loading currently accepts only a successful full response. */
pub(super) const HTTP_OK: u16 = 200;

/** Only fixed categories and numeric status codes may cross into worker diagnostic records. */
#[derive(Clone, Copy)]
pub(in crate::runtime) enum Failure {
    /** The response passed all source gates. */
    Loaded,
    /** The requested address was not an admitted HTTP(S) URL. */
    Address,
    /** The initial script did not share its creator's origin. */
    Origin,
    /** An applicable source policy refused acquisition. */
    Policy,
    /** The server returned a non-success status. */
    Status(u16),
    /** A binary or attachment response cannot supply executable source. */
    BodyKind,
    /** The source exceeded the admitted byte budget. */
    SourceLimit,
    /** A response lacked a supported JavaScript MIME type. */
    Mime,
    /** No resident context or bounded queue capacity remained. */
    Capacity,
    /** Source acquisition failed before an acceptable response existed. */
    Transport,
    /** The absolute network or service deadline expired. */
    Deadline,
    /** The current control permit no longer authorized the operation. */
    Authority,
    /** A process, cancelled runtime or private transport could not continue. */
    Service,
    /** Real JavaScript execution failed, without logging its source-derived exception. */
    Execution,
    /** Script deadlines differ from allocation exhaustion and require a different investigation. */
    ScriptTime,
    /** Instruction work exhausted its allowance before the task returned. */
    ScriptOperations,
}

impl Failure {
    /** Error text is page-controlled; classify only known codes and trusted service presence. */
    pub fn engine(error: &EngineError) -> Self {
        match error.code.as_str() {
            "permission_denied" | "control_denied" => Self::Authority,
            "timeout" => Self::Deadline,
            "limit_exceeded" => Self::Capacity,
            _ if error.service.is_some() => Self::Service,
            _ => Self::Transport,
        }
    }

    /** Non-transport VM failures are execution failures; limits and revoked authority stay distinct. */
    pub fn execution(error: &EngineError) -> Self {
        if error.code == "limit_exceeded" {
            match error.message.as_str() {
                "Script execution stopped: time limit exceeded." => return Self::ScriptTime,
                "Script execution stopped: operation limit exceeded." => {
                    return Self::ScriptOperations;
                }
                _ => {}
            }
        }
        match Self::engine(error) {
            Self::Transport => Self::Execution,
            failure => failure,
        }
    }

    /** Formatting cannot include any caller-supplied text, even an unexpected error code. */
    fn detail(self) -> String {
        match self {
            Self::Loaded => "loaded".into(),
            Self::Address => "address".into(),
            Self::Origin => "origin".into(),
            Self::Policy => "policy".into(),
            Self::Status(status) => format!("status:{status}"),
            Self::BodyKind => "body-kind".into(),
            Self::SourceLimit => "source-limit".into(),
            Self::Mime => "mime".into(),
            Self::Capacity => "capacity".into(),
            Self::Transport => "transport".into(),
            Self::Deadline => "deadline".into(),
            Self::Authority => "authority".into(),
            Self::Service => "service".into(),
            Self::Execution => "execution".into(),
            Self::ScriptTime => "script-time".into(),
            Self::ScriptOperations => "script-operations".into(),
        }
    }
}

/** Initial and imported sources have separate trace events under the same privacy contract. */
pub(in crate::runtime) fn record(initial: bool, started: Instant, failure: Failure) {
    let event = if initial {
        "worker.load"
    } else {
        "worker.import"
    };
    perf::record(event, started, || failure.detail());
}

/** Execution and admission failures never append the exception text or a worker's name. */
pub(in crate::runtime) fn execution_failure(started: Instant, failure: Failure) {
    perf::record("worker.execute", started, || failure.detail());
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Trusted budget failures have distinct diagnoses without retaining arbitrary text. */
    #[test]
    fn execution_budget_categories_do_not_copy_error_text() {
        for (message, expected) in [
            (
                "Script execution stopped: time limit exceeded.",
                "script-time",
            ),
            (
                "Script execution stopped: operation limit exceeded.",
                "script-operations",
            ),
            ("page-controlled-private-detail", "capacity"),
        ] {
            let error = EngineError::new("limit_exceeded", message, "Stop the task.");
            assert_eq!(Failure::execution(&error).detail(), expected);
        }
    }
}
