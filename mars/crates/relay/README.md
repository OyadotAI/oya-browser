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

Measured on 2026-10-28 through `examples/fetch.rs` against
`https://tls.peet.ws/api/all`, with the vendored Rustls (`vendor/rustls`, see its
`MARS.md`) shaping the hello:

| Signal | Relay (measured) | Chrome (published values, not measured here) |
|:---|:---|:---|
| Akamai HTTP/2 | `1:65536;2:0;4:6291456;6:262144\|15663105\|0\|m,a,s,p` (hash `52d84b11737d980aef856699f885ca86`) | identical |
| HEADERS priority | weight 256, exclusive, depends on 0 | identical |
| JA4 | `t13d1516h2_8daaf6152771_d8a2da3f94cd` | identical |
| JA3 | GREASE, 15 suites, 16 extensions, randomized order, e.g. `771,4865-4866-4867-49195-49199-49196-49200-52393-52392-49171-49172-156-157-47-53,43-0-35-10-65281-23-5-16-11-51-45-18-13-27-17613-65037,4588-29-23-24,0` | changes per connection, as here |

The shaping adds what stock Rustls cannot send: GREASE in suites, groups, key shares, versions
and two extensions; `renegotiation_info` (65281) in place of the SCSV; an empty
`signed_certificate_timestamp` (18); ALPS `application_settings` (17613) for h2, with the
client EncryptedExtensions message a server that accepts it waits for; a GREASE
`encrypted_client_hello` outer (65037) from random bytes; and Chrome's six legacy suites
(`c013 c014 009c 009d 002f 0035`) advertised after the nine Rustls runs. A server that picks a
legacy suite fails the Rustls handshake and legacy.rs retries with the kRSA GCM pair; the CBC
four have no path yet. Rustls still places the ECH extension just before the last GREASE where
Chrome shuffles it, and the GREASE ECH payload is a fixed 208 bytes.

Google's search interstitial (the "Please click here if you are not redirected" page with
its botguard script) is served to this hello and to the stock one alike, and to any client
without a passed challenge; it is Google's JavaScript gate, not a fingerprint verdict. What
decides results versus `/sorry/` is the token the page script computes, so that is a V9 matter.

Cloudflare still answers `stackoverflow.com/questions` with its challenge (403)
while header-only checks now pass (reddit.com serves the real page instead of
its block page).

Run `cargo run --manifest-path mars/Cargo.toml -p mars-relay --example policy`
from the repository root for the no-network public consumer example.
