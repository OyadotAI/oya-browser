# Vendored rustls 0.23.46, with one addition

The 0.23.46 security update was imported from the official registry crate
(`https://static.crates.io/crates/rustls/rustls-0.23.46.crate`, SHA-256
`48e13bd8c0e9365c43cfa5c9e8f9ad49d3c8444926c9aac819e0e4dc503c8fdf`).
The complete upstream delta, including its regression tests, was merged over the
existing shaping changes. This includes bounded Brotli decompression, redacted
session debug output, authenticated key-exchange hints and ECH rejection before
buffered application data is sent. See the upstream [release notes](https://github.com/rustls/rustls/releases/tag/v/0.23.46).

This is the registry crate `rustls` 0.23.46 (`src/`, `Cargo.toml`, `build.rs`, licences),
reached through `[patch.crates-io]` in `mars/Cargo.toml`, with one additive change:
`ClientHelloShaping` (`src/client/shaping.rs`, a `pub hello_shaping` field on `ClientConfig`).
With the default value the crate writes byte for byte what upstream writes. With Relay's value
(`crates/relay/src/network/web/hello.rs`) the ClientHello carries what Chrome's does and stock
rustls cannot send: GREASE values, `renegotiation_info` in place of the SCSV suite,
`signed_certificate_timestamp`, a GREASE `encrypted_client_hello` outer, ALPS (the offer and
the client EncryptedExtensions message an accepting server waits for; `src/client/tls13.rs`),
and Chrome's legacy suites advertised after the real ones. Google serves its bot interstitial to the stock hello and
results to the shaped one; that is the whole reason for the fork.

## What changed, by file

- `src/client/shaping.rs` (new): the public struct, the per-handshake GREASE values.
- `src/client/client_conn.rs`, `src/client/builder.rs`: the field and its default.
- `src/client/common.rs`: `ClientHelloDetails.grease`, drawn once so a retried hello repeats it.
- `src/client/hs.rs`: applies the shaping while building the hello; the GREASE ECH outer from
  random bytes shares `EchStatus::Grease`, so retry configs are ignored and a retry carries it;
  extra extensions count as sent, so a server answering one is not "unsolicited".
  A HelloRetryRequest selecting a group emits exactly that key share; GREASE key shares
  remain on initial and cookie-only retry hellos.
- `src/msgs/handshake.rs`: `ClientExtensions.grease` and `.extra` with their encoding,
  `SupportedProtocolVersions.grease`, and `CertificateExtensions` reads (and ignores) SCTs.
- `Cargo.toml`: the `[[example]]` and `[[bench]]` targets whose files the registry crate does not
  ship are dropped; nothing else.
- Test fixture literals in `src/server/test.rs` and `src/msgs/handshake_test.rs` name the new
  `grease: None` field.

## Diffing against upstream

    diff -r ~/.cargo/registry/src/*/rustls-0.23.46/src mars/vendor/rustls/src

Upstream's own unit tests need `testdata/` and `test-ca/`, which the registry tarball does not
ship, so they are not run here; Relay's `hello/tests_hello.rs` pins the shaped hello (suites,
extensions, JA4, GREASE freshness, ECH shape) and handshakes it against a loopback rustls server
on TLS 1.3, TLS 1.2 only, and through a HelloRetryRequest.

## Ceiling

ALPS carries empty client settings and the server's settings are read and dropped; nothing
above TLS uses them. The four CBC suites are advertised but no path runs them (legacy.rs serves
the two kRSA GCM ones). Upgrading rustls means re-applying this change; a Relay-owned TLS client
would retire it.
