/** Opt-in request outcomes retain fixed native categories without recording page secrets. */
use super::ScriptRequest;
use crate::protocol::EngineError;
use crate::runtime::supervisor::perf;
use std::time::Instant;
use url::Url;

/** The coordinator gate that failed, not a guessed transport cause. */
#[derive(Clone, Copy)]
pub(super) enum FailureStage {
    /** Worker connect policy rejected this destination. */
    Policy,
    /** The request mode refused the destination origin. */
    Mode,
    /** Preflight acquisition or response admission failed. */
    Preflight,
    /** Governed acquisition failed before producing a response. */
    Acquire,
    /** The response did not authorize CORS exposure. */
    Cors,
    /** Redirect destination resolution or policy refused continuation. */
    Redirect,
    /** The bounded redirect chain was exhausted. */
    RedirectLimit,
}

impl FailureStage {
    /** Only static labels can reach the log sink. */
    fn name(self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::Mode => "mode",
            Self::Preflight => "preflight",
            Self::Acquire => "acquire",
            Self::Cors => "cors",
            Self::Redirect => "redirect",
            Self::RedirectLimit => "redirect_limit",
        }
    }
}

/** A copyable snapshot deliberately cannot retain URL, headers, body or exception text. */
#[derive(Clone, Copy)]
pub(super) struct RequestDiagnostic {
    /** Native fetch/XHR identity; module acquisitions have no such ID. */
    id: Option<u32>,
    /** A supported method's static name, never arbitrary caller text. */
    method: &'static str,
    /** A fixed public endpoint family; all other paths are indistinguishable. */
    endpoint: &'static str,
    /** Origin relationship is useful without retaining either origin's text. */
    same_origin: bool,
}

impl RequestDiagnostic {
    /** Resolve only privacy-safe categories while the governed request is available. */
    pub fn new(base: &Url, request: &ScriptRequest) -> Self {
        Self {
            id: request.request_id,
            method: request.method.name(),
            endpoint: endpoint(&request.url),
            same_origin: base.origin() == request.url.origin(),
        }
    }

    /** Policy failures stay opaque to scripts and become visible only in the opt-in sink. */
    pub fn refused(self, stage: FailureStage) {
        self.record(Instant::now(), stage, "refused");
    }

    /** Unknown error codes and all error messages are discarded rather than escaped. */
    pub fn error(self, started: Instant, stage: FailureStage, error: &EngineError) {
        self.record(started, stage, safe_code(error));
    }

    /** HTTP errors still resolve fetch; record their status without copying headers or content. */
    pub fn response(self, started: Instant, stage: FailureStage, status: u16) {
        perf::record("request.response", started, || self.response_detail(stage, status));
    }

    /** Response admission is separate from receipt, including preflight and opaque responses. */
    fn response_detail(self, stage: FailureStage, status: u16) -> String {
        format!("{}\tstatus={status}", self.detail(stage, "received"))
    }

    /** The existing sink is disabled unless the coordinator was launched with MARS_PERF. */
    fn record(self, started: Instant, stage: FailureStage, code: &'static str) {
        perf::record("request.failure", started, || self.detail(stage, code));
    }

    /** Formatting accepts only bounded native numeric identities and fixed categories. */
    fn detail(self, stage: FailureStage, code: &'static str) -> String {
        let id = self.id.map_or_else(|| "none".into(), |id| id.to_string());
        format!(
            "id={id}\tstage={}\tcode={code}\tmethod={}\tendpoint={}\tsame_origin={}",
            stage.name(),
            self.method,
            self.endpoint,
            self.same_origin
        )
    }
}

/** Match exact public service hosts; a suffix/lookalike cannot reveal a private path. */
fn endpoint(url: &Url) -> &'static str {
    if !matches!(
        url.host_str(),
        Some("www.youtube.com" | "youtube.com" | "m.youtube.com")
    ) {
        return "other";
    }
    match url.path() {
        "/generate_204" => "generate_204",
        path if path.starts_with("/youtubei/") => "youtube_innertube",
        _ => "other",
    }
}

/** No unrecognized or page-derived code is copied into the private diagnostic file. */
fn safe_code(error: &EngineError) -> &'static str {
    match error.code.as_str() {
        "network_error" => "network_error",
        "tls_error" => "tls_error",
        "timeout" => "timeout",
        "navigation_timeout" => "navigation_timeout",
        "permission_denied" => "permission_denied",
        "control_denied" => "control_denied",
        "limit_exceeded" => "limit_exceeded",
        "invalid_request" => "invalid_request",
        "unsupported" => "unsupported",
        "submission_uncertain" => "submission_uncertain",
        _ if error.service.is_some() => "service_error",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Tokens, arbitrary paths and deceptive hosts never survive category selection. */
    #[test]
    fn endpoint_categories_discard_private_address_components() {
        for (raw, expected) in [
            (
                "https://www.youtube.com/youtubei/v1/browse?key=secret",
                "youtube_innertube",
            ),
            (
                "https://www.youtube.com/generate_204?secret#private",
                "generate_204",
            ),
            ("https://www.youtube.com/account/private", "other"),
            (
                "https://www.youtube.com.evil.test/youtubei/private",
                "other",
            ),
            ("https://private.test/generate_204", "other"),
        ] {
            assert_eq!(endpoint(&Url::parse(raw).unwrap()), expected);
        }
    }

    /** Logging distinguishes real network codes without copying unknown codes or messages. */
    #[test]
    fn failure_record_contains_only_fixed_labels_and_numeric_identity() {
        let diagnostic = RequestDiagnostic {
            id: Some(12),
            method: "POST",
            endpoint: "youtube_innertube",
            same_origin: true,
        };
        let secret = EngineError::new("private\tcode", "private body", "private URL");
        assert_eq!(
            diagnostic.detail(FailureStage::Acquire, safe_code(&secret)),
            "id=12\tstage=acquire\tcode=other\tmethod=POST\tendpoint=youtube_innertube\tsame_origin=true"
        );
        let network = EngineError::new("network_error", "private body", "private URL");
        assert_eq!(safe_code(&network), "network_error");
    }

    /** HTTP rejection statuses remain visible without turning them into transport failures. */
    #[test]
    fn response_record_retains_status_and_native_request_identity() {
        let diagnostic = RequestDiagnostic {
            id: Some(7),
            method: "POST",
            endpoint: "youtube_innertube",
            same_origin: true,
        };
        assert_eq!(diagnostic.response_detail(FailureStage::Acquire, 403),
            "id=7\tstage=acquire\tcode=received\tmethod=POST\tendpoint=youtube_innertube\tsame_origin=true\tstatus=403");
    }
}
