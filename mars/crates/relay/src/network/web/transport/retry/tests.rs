/** Pure recovery decisions require no socket, resolver, network or TLS server fixture. */
use super::*;
use std::{cell::Cell, time::Duration};

/** Synthetic admitted peers let family preference be asserted without opening connections. */
fn peers() -> [SocketAddr; 3] {
    [
        "[2001:db8::1]:443".parse().unwrap(),
        "[2001:db8::2]:443".parse().unwrap(),
        "192.0.2.1:443".parse().unwrap(),
    ]
}

/** A fresh shared deadline avoids waiting or timer mocks in ordinary admission fixtures. */
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(60)
}

/** A reset remains typed until the pre-request recovery decision. */
fn reset() -> Failure {
    Failure::Handshake(io::ErrorKind::ConnectionReset.into())
}

/** Family diversity wins over DNS order; remaining choices are distinct peer then same peer. */
#[test]
fn alternate_peer_selection_never_invents_an_address() {
    let addresses = peers();
    assert_eq!(preferred_peer(&addresses, addresses[0]), addresses[2]);
    assert_eq!(preferred_peer(&addresses[..2], addresses[0]), addresses[1]);
    assert_eq!(preferred_peer(&addresses[..1], addresses[0]), addresses[0]);
}

/** A raw reset can recover through exactly one modern candidate and never selects RSA. */
#[test]
fn reset_retries_modern_tls_once_on_an_admitted_peer() {
    let addresses = peers();
    let calls = Cell::new(0);
    let answer = recover(
        Err(reset()),
        &addresses,
        addresses[0],
        deadline(),
        |peer| {
            calls.set(calls.get() + 1);
            assert_eq!(peer, addresses[2]);
            Ok(7)
        },
        || panic!("reset cannot select RSA"),
    )
    .unwrap();
    assert_eq!(answer, 7);
    assert_eq!(calls.get(), 1);
}

/** Even an alert eligible for the old fallback cannot create a third TLS candidate. */
#[test]
fn second_candidate_reset_or_alert_is_terminal() {
    let addresses = peers();
    for error in [
        io::ErrorKind::ConnectionReset.into(),
        io::Error::new(
            io::ErrorKind::InvalidData,
            rustls::Error::AlertReceived(rustls::AlertDescription::HandshakeFailure),
        ),
    ] {
        assert!(
            recover::<()>(
                Err(reset()),
                &addresses,
                addresses[0],
                deadline(),
                |_| Err(Failure::Handshake(error)),
                || panic!("no third candidate")
            )
            .is_err()
        );
    }
}

/** Existing first-attempt handshake_failure recovery remains available without modern retry. */
#[test]
fn initial_explicit_alert_keeps_the_existing_legacy_path() {
    let addresses = peers();
    let failure = Failure::Handshake(io::Error::new(
        io::ErrorKind::InvalidData,
        rustls::Error::AlertReceived(rustls::AlertDescription::HandshakeFailure),
    ));
    let result = recover(
        Err(failure),
        &addresses,
        addresses[0],
        deadline(),
        |_| panic!("alert cannot select reset retry"),
        || Ok(9),
    )
    .unwrap();
    assert_eq!(result, 9);
}

/** Certificate failures, malformed TLS and non-reset transport failures never trigger recovery. */
#[test]
fn other_failure_categories_are_terminal() {
    let addresses = peers();
    let errors = [
        io::ErrorKind::UnexpectedEof.into(),
        io::ErrorKind::ConnectionAborted.into(),
        io::ErrorKind::InvalidData.into(),
        io::Error::new(
            io::ErrorKind::InvalidData,
            rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer),
        ),
        io::Error::new(
            io::ErrorKind::ConnectionReset,
            rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer),
        ),
    ];
    for error in errors {
        assert!(
            recover::<()>(
                Err(Failure::Handshake(error)),
                &addresses,
                addresses[0],
                deadline(),
                |_| panic!("only raw resets retry"),
                || panic!("only explicit handshake_failure selects RSA")
            )
            .is_err()
        );
    }
}

/** A lease-triggered reset at expiration is reported as timeout without opening another socket. */
#[test]
fn expired_deadline_prevents_recovery_and_success_handoff() {
    let addresses = peers();
    let expired = Instant::now() - Duration::from_secs(1);
    for result in [Err(reset()), Ok(())] {
        let error = recover(
            result,
            &addresses,
            addresses[0],
            expired,
            |_| panic!("expired retry"),
            || panic!("expired fallback"),
        )
        .unwrap_err();
        assert_eq!(error.code, "timeout");
    }
}
