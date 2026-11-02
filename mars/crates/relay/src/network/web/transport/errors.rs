/** Transport errors expose only typed failure categories and operation stages, never destination or payload data. */
use crate::protocol::EngineError;
use std::io::{Error, ErrorKind};

/** The failing operation is supplied by its callsite rather than inferred from an OS error string. */
#[derive(Clone, Copy)]
pub(super) enum Stage {
    /// Allocating the bounded resolver helper failed before DNS began.
    ResolverStart,
    /// The system resolver could not return addresses.
    Resolve,
    /// An exact admitted address failed during TCP connection establishment.
    Connect,
    /// Socket options failed before the next timed I/O operation.
    SocketOptions,
    /// TLS negotiation failed before application data transmission.
    Handshake,
    /// Sending application request bytes failed.
    Write,
    /// Receiving application response bytes failed.
    Read,
}

/** Preserve error codes and timeout classification while making the failed stage understandable. */
pub(super) fn io_error(stage: Stage, error: Error) -> EngineError {
    if error.kind() == ErrorKind::InvalidData {
        return super::tls_failure(stage, &error);
    }
    if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) {
        return EngineError::new("timeout", stage.timeout_message(), "Retry the destination.");
    }
    EngineError::new(
        "network_error",
        stage.network_message(error.kind()),
        "Check the URL and retry.",
    )
}

impl Stage {
    /** A deadline at a known I/O stage does not imply that the peer refused or reset the connection. */
    fn timeout_message(self) -> &'static str {
        match self {
            Self::ResolverStart => "Starting the address lookup timed out.",
            Self::Resolve => "The destination's address could not be resolved in time.",
            Self::Connect => "The connection attempt timed out.",
            Self::SocketOptions => "Configuring the network socket timed out.",
            Self::Handshake => "The TLS handshake timed out.",
            Self::Write => "Sending the web request timed out.",
            Self::Read => "Waiting for the web response timed out.",
        }
    }

    /** Unknown failures retain their known phase without inventing an underlying cause. */
    fn network_message(self, kind: ErrorKind) -> &'static str {
        match self {
            Self::ResolverStart => "The address lookup could not be started.",
            Self::Resolve => "The destination's address could not be resolved.",
            Self::SocketOptions => "The network socket could not be configured.",
            Self::Connect => match kind {
                ErrorKind::ConnectionRefused => "The destination refused the connection.",
                ErrorKind::ConnectionReset => "The connection was reset while connecting.",
                ErrorKind::ConnectionAborted => "The connection attempt was aborted.",
                ErrorKind::HostUnreachable => "The destination is unreachable.",
                ErrorKind::NetworkUnreachable => "The destination network is unreachable.",
                ErrorKind::PermissionDenied => "The system denied permission to connect.",
                ErrorKind::AddrNotAvailable => {
                    "A local address was unavailable for the connection."
                }
                _ => "The connection attempt failed.",
            },
            Self::Handshake => match kind {
                ErrorKind::UnexpectedEof | ErrorKind::BrokenPipe | ErrorKind::NotConnected => {
                    "The connection closed before the TLS handshake completed."
                }
                ErrorKind::ConnectionReset => "The connection was reset during the TLS handshake.",
                ErrorKind::ConnectionAborted => {
                    "The connection was aborted during the TLS handshake."
                }
                ErrorKind::PermissionDenied => "The system denied access during the TLS handshake.",
                _ => "The TLS handshake failed.",
            },
            Self::Write => match kind {
                ErrorKind::ConnectionReset => {
                    "The connection was reset while sending the web request."
                }
                ErrorKind::ConnectionAborted => {
                    "The connection was aborted while sending the web request."
                }
                ErrorKind::BrokenPipe
                | ErrorKind::UnexpectedEof
                | ErrorKind::NotConnected
                | ErrorKind::WriteZero => "The connection closed before the web request was sent.",
                ErrorKind::PermissionDenied => {
                    "The system denied permission to send the web request."
                }
                _ => "The web request could not be sent.",
            },
            Self::Read => match kind {
                ErrorKind::ConnectionReset => {
                    "The connection was reset while reading the web response."
                }
                ErrorKind::ConnectionAborted => {
                    "The connection was aborted while reading the web response."
                }
                ErrorKind::UnexpectedEof | ErrorKind::BrokenPipe | ErrorKind::NotConnected => {
                    "The connection closed before the web response was complete."
                }
                ErrorKind::PermissionDenied => {
                    "The system denied permission to read the web response."
                }
                _ => "Reading the web response failed.",
            },
        }
    }

    /** Invalid TLS data is distinct from a typed certificate or protocol-suite refusal. */
    pub(super) fn tls_message(self) -> &'static str {
        match self {
            Self::Handshake => "The TLS handshake failed.",
            Self::Write => "The secure connection failed while sending the web request.",
            Self::Read => {
                "The secure connection returned invalid TLS data while reading the web response."
            }
            Self::ResolverStart => "The address lookup could not be started.",
            Self::Resolve => "The address lookup returned invalid data.",
            Self::Connect => "The connection attempt failed.",
            Self::SocketOptions => "The network socket settings were rejected.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Stage and kind alone determine public text; arbitrary underlying details never escape. */
    #[test]
    fn failures_identify_the_operation_without_exposing_underlying_error_details() {
        for (stage, kind, expected) in [
            (
                Stage::Resolve,
                ErrorKind::Other,
                "The destination's address could not be resolved.",
            ),
            (
                Stage::Connect,
                ErrorKind::ConnectionRefused,
                "The destination refused the connection.",
            ),
            (
                Stage::Connect,
                ErrorKind::ConnectionReset,
                "The connection was reset while connecting.",
            ),
            (
                Stage::Connect,
                ErrorKind::NetworkUnreachable,
                "The destination network is unreachable.",
            ),
            (
                Stage::Connect,
                ErrorKind::PermissionDenied,
                "The system denied permission to connect.",
            ),
            (
                Stage::Handshake,
                ErrorKind::UnexpectedEof,
                "The connection closed before the TLS handshake completed.",
            ),
            (
                Stage::Write,
                ErrorKind::BrokenPipe,
                "The connection closed before the web request was sent.",
            ),
            (
                Stage::Read,
                ErrorKind::ConnectionReset,
                "The connection was reset while reading the web response.",
            ),
            (
                Stage::Read,
                ErrorKind::UnexpectedEof,
                "The connection closed before the web response was complete.",
            ),
            (
                Stage::Read,
                ErrorKind::Other,
                "Reading the web response failed.",
            ),
        ] {
            let error = io_error(
                stage,
                Error::new(kind, "secret URL https://private.example/path and payload"),
            );
            assert_eq!(error.code, "network_error");
            assert_eq!(error.message, expected);
        }
    }

    /** Both OS timeout representations retain the existing timeout code at every operation stage. */
    #[test]
    fn timeouts_remain_distinct_from_connection_failures() {
        for stage in [
            Stage::ResolverStart,
            Stage::Resolve,
            Stage::Connect,
            Stage::SocketOptions,
            Stage::Handshake,
            Stage::Write,
            Stage::Read,
        ] {
            for kind in [ErrorKind::TimedOut, ErrorKind::WouldBlock] {
                let error = io_error(stage, Error::from(kind));
                assert_eq!(error.code, "timeout");
                assert_eq!(error.message, stage.timeout_message());
            }
        }
    }

    /** Certificate verification and cipher refusal preserve their supported copy and retry decisions. */
    #[test]
    fn typed_tls_failures_keep_certificate_refusals_separate_from_other_handshake_errors() {
        let certificate = Error::new(
            ErrorKind::InvalidData,
            rustls::Error::InvalidCertificate(rustls::CertificateError::Expired),
        );
        assert!(!super::super::retry_with_rsa(&certificate));
        let error = io_error(Stage::Handshake, certificate);
        assert_eq!(error.code, "tls_error");
        assert_eq!(
            error.message,
            "The server's certificate could not be verified."
        );
        let refusal = Error::new(
            ErrorKind::InvalidData,
            rustls::Error::AlertReceived(rustls::AlertDescription::HandshakeFailure),
        );
        assert!(super::super::retry_with_rsa(&refusal));
        let error = io_error(Stage::Handshake, refusal);
        assert_eq!(
            error.message,
            "The server uses an unsupported TLS protocol or cipher suite."
        );
        let error = io_error(
            Stage::Handshake,
            Error::new(ErrorKind::InvalidData, "private TLS data"),
        );
        assert_eq!(error.code, "tls_error");
        assert_eq!(error.message, "The TLS handshake failed.");
    }
}
