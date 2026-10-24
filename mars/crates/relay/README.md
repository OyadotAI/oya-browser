<!-- Relay's independent adoption surface and explicit transport policy. -->
# Relay

Relay owns Mars's bounded HTTP/TLS implementation. Use it without starting the
Mars app, Eye, V9 or a service process:

```toml
[dependencies]
mars-relay = { path = "/path/to/agentchrome/mars/crates/relay" }
```

```rust
use mars_relay::{WebRequest, policy};

let request = WebRequest::get("https://1.1.1.1/document");
request.validate().expect("bounded GET");
let url = policy::web_url(&request.url).expect("HTTP/HTTPS URL syntax");
assert!(policy::public_address("1.1.1.1".parse().unwrap()));
assert_eq!(url.path(), "/document");
```

This example validates data without DNS or socket I/O. `web_url` checks syntax;
it does not by itself authorize a resolved destination. `Prepared::connect`
revalidates requests and URL policy, resolves and checks every address, and
connects to an exact admitted address, including normal TLS verification.
Consuming `Prepared::transmit` sends HTTP application bytes once. The caller
must perform its own application/agent ownership admission before that step.
Only exact explicitly granted loopback origins bypass the public-address
policy. No ambient proxy or credentials are used.

The existing bounded subset includes GET, URL-encoded POST, bounded HTTP/1.1
framing, owned HTTP/2 over TLS (ALPN `h2`, then `http/1.1`), verified TLS,
explicit redirects, gzip/deflate/br/zstd content decoding bounded by the body
budget, a keep-alive pool for HTTP/1.1 GETs, one multiplexed HTTP/2 connection
per origin, and a private RFC 9111 subset cache. The cache is used only when a
request sets `top_level` (its origin is the cache partition); it lives in memory
unless `set_cache_directory` names a folder. `fetch_batch` fetches independent
GET subresources concurrently on a bounded set of threads; on an HTTP/2 origin
they share one connection. It is under development, is not a complete Chromium
networking replacement, and is not published to crates.io. Legacy
`fetch`/`Endpoint` are the separately restricted literal-loopback fixture API.

## Cookies

`CookieJar` follows RFC 6265bis: host-only and `Domain` cookies (a Domain that is
a public suffix, from the compiled-in Public Suffix List, is refused), schemeful
SameSite (Strict, Lax, None; unspecified behaves as Lax), `Max-Age` over
`Expires` with both clamped to 400 days, `Secure`, `HttpOnly`, the `__Secure-`
and `__Host-` prefixes, and segment-aware path matching. It keeps 180 cookies
per registrable domain and 3000 in total (Chromium's limits), evicting the least
recently used. `header` is for HTTP requests; `document_cookie` and
`set_document_cookie` are the `document.cookie` getter and setter, which never
see or replace an HttpOnly cookie. Profile records are versioned: version 2 is
written, version 1 (the old host-only shape) still loads. Records are pure
bytes; persistence and secure storage remain the caller's responsibility, and a
record may be up to `network::constants::COOKIE_PROFILE_BYTES`.

## Looking like Chrome on the wire

Requests carry Chrome 152 on macOS's headers in Chrome's order (`sec-ch-ua*`,
`upgrade-insecure-requests`, `user-agent`, `accept`, `sec-fetch-*`, `referer`,
`accept-encoding`, `accept-language`, `cookie`, and `priority` on HTTP/2).
`WebRequest::initiator` and `destination` let the caller make Sec-Fetch-Site,
Origin, Referer, Accept and the priority match what Chrome would send.

Measured on 2026-10-23 through `examples/fetch.rs` against
`https://tls.peet.ws/api/all`:

| Signal | Relay (measured) | Chrome (published values, not measured here) |
|:---|:---|:---|
| Akamai HTTP/2 | `1:65536;2:0;4:6291456;6:262144\|15663105\|0\|m,a,s,p` (hash `52d84b11737d980aef856699f885ca86`) | identical |
| HEADERS priority | weight 256, exclusive, depends on 0 | identical |
| JA4 | `t13d1012h2_61a7ad8aa9b6_fe7e4c5e2d0e` | `t13d1516h2_...` |
| JA3 | `771,4865-4866-4867-49195-49199-49196-49200-52393-52392-255,11-13-43-27-35-23-51-45-0-16-5-10,29-23-24,0` | GREASE, 15 suites, 16 extensions, randomized order |

The suites Rustls offers are in Chrome's order, followed by the renegotiation
SCSV; signature schemes, X25519 first and certificate compression (Brotli) match
Chrome. What Rustls cannot do, so the TLS fingerprint still differs:

- GREASE values in suites, groups, extensions and versions.
- Chrome's randomized extension order (JA3 changes per connection; JA4 sorts).
- The CBC and static-RSA suites (`c013 c014 009c 009d 002f 0035`): 9 suites
  instead of 15.
- `renegotiation_info` (65281; Rustls sends the SCSV instead),
  `signed_certificate_timestamp` (18), ALPS `application_settings` (17613) and
  ECH GREASE `encrypted_client_hello` (65037; Rustls supports it only with an
  HPKE provider, which ring lacks): 12 extensions instead of 16.
- `X25519MLKEM768` (4588) as the first group and key share (aws-lc-rs only).

Cloudflare still answers `stackoverflow.com/questions` with its challenge (403)
while header-only checks now pass (reddit.com serves the real page instead of
its block page).

Run `cargo run --manifest-path mars/Cargo.toml -p mars-relay --example policy`
from the repository root for the no-network public consumer example.
