# D1: Mars's TLS and HTTP/2 fingerprint compared with Chrome 152 on macOS

Research only. No code was changed. Measured on 2026-10-28 from the working tree on branch
`blink-parity` (rustls 0.23.45, ring provider).

## Summary

- HTTP/2 already matches Chrome exactly: SETTINGS values and order, the connection WINDOW_UPDATE,
  pseudo-header order, the HEADERS priority fields, and the navigation header order and values.
  The Akamai fingerprint is identical (`52d84b11737d980aef856699f885ca86`).
- TLS does not match. Mars's JA4 is `t13d1012h2_61a7ad8aa9b6_fe7e4c5e2d0e`. Chrome 152 on macOS
  computes to `t13d1517h2_8daaf6152771_cb7bf5808d99`. All three JA4 parts differ: 10 suites
  against 15, 12 extensions against 17, and 8 signature schemes against 11. Any JA4 or JA3
  allowlist sees Mars as "not Chrome". Because these are exact-match hashes, closing half the
  gaps buys nothing; the hello has to match completely before it helps.
- For Google specifically, the block happens after the JavaScript challenge, not at the TLS
  handshake. A plain Relay fetch of `https://www.google.com/search?q=rust+tls` gets HTTP 200
  with Google's "enable JavaScript" interstitial (the same page every client gets first). In the
  Mars desktop app, that page's script runs and navigates to `/search?...&sei=...`, and that
  second request is answered with `429` and the `/sorry/` "unusual traffic" page. So Google
  scored the second request, which carries whatever the challenge script produced, Mars's
  missing high-entropy client hints, and the non-Chrome TLS hello. Which of these decides it
  cannot be separated without running Chrome next to Mars, which this task does not allow.

## How the values were observed

1. The desktop app route (`mars desktop --agent --agent-control --ephemeral --url
   https://tls.peet.ws/api/all`) did not work: Mars does not render an `application/json`
   response as a page, so `page.inspect` and `page.summary` return `no_page`. HTML pages work
   (`https://example.com` loaded). A `page.navigate` from the agent channel returned
   `control_denied` in this mode; `--url` is what loads the page.
2. Instead, the hello was measured through Relay's own example, which goes through the same
   `Prepared::connect` path and the same shared `ClientConfig` the desktop uses:
   `cargo run --manifest-path mars/Cargo.toml -p mars-relay --example fetch https://tls.peet.ws/api/all`
   (three runs). Values below marked "measured" come from those responses.
3. The Google sequence was observed with the same `fetch` example (first request) and with the
   desktop app and `page.summary` / `page.network` / `page.inspect` (the follow-up request).
4. Chrome's values come from curl-impersonate's capture of Chrome 152 on macOS 26
   (`tests/signatures/chrome152_macos.yaml`, source [S1]) and BoringSSL's headers [S2] for
   code points. Chrome's JA4 was computed from [S1] with the JA4 algorithm; the same script
   reproduces Mars's measured JA4 exactly and reproduces the widely published pre-ML-DSA
   Chrome JA4 `t13d1516h2_8daaf6152771_d8a2da3f94cd`, so the method is right. Chrome was not
   launched.

## What Mars sends

Code: `crates/relay/src/network/web/hello.rs` (suites, schemes, groups, ALPN),
`hello/hybrid.rs` (X25519MLKEM768 over our own ML-KEM), `h2/frame.rs` (`preface`),
`h2/mod.rs` (`request_fields`), `h2/state.rs` (`write_headers`), `h2/hpack.rs` (`encode`),
`fields.rs` (headers, User-Agent, client hints).

TLS (measured):

- Record and `client_version`: 0x0303 (771). Negotiated TLS 1.3. 32-byte random session id.
- Cipher suites, in order: 1301, 1302, 1303, c02b, c02f, c02c, c030, cca9, cca8, then 00ff
  (`TLS_EMPTY_RENEGOTIATION_INFO_SCSV`). No GREASE.
- Extensions (12, order randomized per connection by rustls's `order_seed`; three runs gave
  three JA3 hashes): server_name (0), status_request (5), supported_groups (10),
  ec_point_formats (11), signature_algorithms (13), ALPN (16), extended_master_secret (23),
  compress_certificate (27, brotli), session_ticket (35, empty), supported_versions (43:
  1.3, 1.2), psk_key_exchange_modes (45: psk_dhe_ke), key_share (51). pre_shared_key (41) on
  resumption.
- Supported groups: 4588 (X25519MLKEM768), 29 (X25519), 23 (P-256), 24 (P-384).
- Key shares: X25519MLKEM768 (1216 bytes) and X25519 (32 bytes).
- Signature algorithms: 0403, 0804, 0401, 0503, 0805, 0501, 0806, 0601.
- ALPN: h2, http/1.1.
- Certificate compression: brotli only.
- ECH: none. ALPS: none. SCT request: none. renegotiation_info: none (SCSV instead).
- JA3 (one run): `771,4865-4866-4867-49195-49199-49196-49200-52393-52392-255,45-27-0-16-11-10-35-51-13-23-43-5,4588-29-23-24,0`.
- JA4: `t13d1012h2_61a7ad8aa9b6_fe7e4c5e2d0e`.

HTTP/2 (measured):

- SETTINGS: 1=65536, 2=0, 4=6291456, 6=262144, in that order.
- WINDOW_UPDATE on stream 0: 15663105.
- No PRIORITY frames. HEADERS carries the priority flag: exclusive, depends on 0, weight 256
  for a document (per-destination weights in `fields::priority`).
- Pseudo-headers: `:method`, `:authority`, `:scheme`, `:path`.
- Akamai: `1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p`.
- HPACK: every field is either a fully indexed static entry or a literal "without indexing",
  never Huffman-coded and never added to the dynamic table. The first navigation's HEADERS
  block was 594 bytes.

Navigation headers (measured, lowercase, in this order): `sec-ch-ua`, `sec-ch-ua-mobile`,
`sec-ch-ua-platform`, `upgrade-insecure-requests`, `user-agent`, `accept`, `sec-fetch-site`,
`sec-fetch-mode`, `sec-fetch-user`, `sec-fetch-dest`, `accept-encoding`, `accept-language`,
(`cookie`), `priority`. HTTP/1.1 uses Title-Case names except `sec-ch-*`, after `Host` and
`Connection`.

- User-Agent: `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/152.0.0.0 Safari/537.36`.
- `sec-ch-ua`: `"Chromium";v="152", "Not?A_Brand";v="24", "Google Chrome";v="152"`.
- `sec-ch-ua-mobile: ?0`, `sec-ch-ua-platform: "macOS"`.
- No high-entropy hints, ever: Relay does not read `Accept-CH` (no match for `accept-ch` in
  `crates/relay/src`).
- TCP (measured, p0f `4:50+?:0:1400:65535,6:mss,nop,ws,nop,nop,ts,sok,eol+1:df,ecn:0`): the
  macOS kernel stack, which is what Chrome on macOS also uses. No difference.

## Difference table

"Can rustls match" means with rustls 0.23.45's public API and our own provider code, no fork.

| Item | Mars | Chrome 152 macOS | Source | Can rustls match |
|:---|:---|:---|:---|:---|
| Cipher suites | 9 + SCSV 00ff, no GREASE | GREASE, 1301, 1302, 1303, c02b, c02f, c02c, c030, cca9, cca8, c013, c014, 009c, 009d, 002f, 0035 | S1 | No. rustls appends the SCSV itself and has no CBC suites. Needs a custom ClientHello (or a rustls fork) that offers the six legacy suites. 009c and 009d already exist in `legacy.rs`; c013, c014, 002f and 0035 are CBC and would need an implementation or a refusal path if a server picks them. |
| GREASE (suites, extensions x2, groups, key share, versions) | none | present | S1 (`grease: true`), BoringSSL | No. rustls has no GREASE. Needs a fork patch in `client/hs.rs` and the extension encoder. |
| Extension set | 12 | 17 plus 2 GREASE: adds renegotiation_info (65281), signed_certificate_timestamp (18), application_settings (17613), encrypted_client_hello (65037), trust_anchors (0xca34) | S1, S2 | Partly. ECH GREASE: yes with work (below). The other four: no, needs a fork or custom hello. |
| Extension order | random per connection | random per connection (`tls_permute_extensions: true`), GREASE first and last | S1 | Already matches in kind. JA4 sorts, so order only matters for JA3, which Chrome randomizes anyway. |
| renegotiation_info | SCSV in suites | extension 65281, empty | S1 | No. rustls always uses the SCSV. Fork. Server side needs nothing new: servers echo the same reply for either form. |
| signed_certificate_timestamp | absent | empty request | S1 | No. Fork, and rustls must then accept SCT data in the server's reply. |
| ALPS (application_settings 17613) | absent | present, protocol h2 | S1, S2 | No. Needs a fork and real protocol work: when the server agrees (Google does), the client must send its own settings in a client EncryptedExtensions message. |
| ECH GREASE | absent | present | S1 | Yes, with work. rustls has `EchMode::Grease`, but it needs an `Hpke` implementation (ring has none; write X25519-HKDF-SHA256 with AES-128-GCM over ring, or use aws-lc-rs) and `with_ech` forces TLS 1.3 only. Workable as a TLS 1.3 config plus a TLS 1.2 retry config, the same retry pattern the kRSA path in `legacy.rs` uses. |
| trust_anchors (0xca34) | absent | present | S1, S2 | No. Fork or custom hello. Behaviour when a server answers it needs reading first. |
| Signature algorithms | 8 | 0904, 0905, 0906 (ML-DSA-44/65/87) then Mars's 8 in the same order | S1, S2 | Yes. rustls 0.23.45 knows `ML_DSA_44/65/87`. Add mapping entries whose verifier refuses (or verifies, if a Rust ML-DSA is added); no public server picks ML-DSA today. |
| Supported groups | 4588, 29, 23, 24 | GREASE, 4588, 29, 23, 24 | S1 | Matches except GREASE. |
| Key shares | 4588, 29 | GREASE (1 byte), 4588, 29 | S1 | Matches except GREASE. |
| supported_versions | 1.3, 1.2 | GREASE, 1.3, 1.2 | S1 | Matches except GREASE. |
| ALPN | h2, http/1.1 | h2, http/1.1 | S1 | Matches. |
| Certificate compression | brotli | brotli | S1 | Matches. |
| psk_key_exchange_modes, EMS, session_ticket, status_request, ec_point_formats | present | present | S1 | Match. |
| JA4 | `t13d1012h2_61a7ad8aa9b6_fe7e4c5e2d0e` | `t13d1517h2_8daaf6152771_cb7bf5808d99` (computed) | measured, S1 | Only after every row above is fixed. |
| HTTP/2 SETTINGS, WINDOW_UPDATE, pseudo-header order, HEADERS priority | see above | identical | measured, S1 | Matches (own H2 code). |
| HPACK encoding | no Huffman, no dynamic table | Huffman and incremental indexing | Chromium net/spdy (HPACK encoder defaults) | Own code. Detectable only by passive analysis of the header block bytes; low priority. |
| Navigation header order and values | see above | identical, plus `cache-control: max-age=0` on a reload | S1 | Matches. Mars sends `cache-control` only for a POST navigation; check the reload path sends it too. |
| High-entropy client hints | never sent | sent to origins that ask. Google's `www.google.com` response asks for Sec-CH-UA-Arch, -Bitness, -Full-Version, -Full-Version-List, -Model, -Platform-Version, -WoW64, -Form-Factors, plus Downlink, RTT and Sec-CH-Prefers-Color-Scheme | Google response headers (curl, 2026-10-28) | Not TLS. Own code in `fields.rs` plus a per-origin Accept-CH store. |
| Google-only headers | none | `x-client-data` (variations; not in Incognito), `x-browser-channel`, `x-browser-year`, `x-browser-copyright`, `x-browser-validation` | S3, S4 | Not TLS. `x-browser-validation` is a hash over a key extracted from Chrome binaries and the User-Agent (third-party reverse engineering only). Shipping it means embedding Google's keys: a maintainer decision, not recommended here. An ephemeral window without `x-client-data` is consistent with Incognito. |
| HTTP/3 | none | Chrome moves to h3 after `alt-svc: h3=":443"` (Google sends it) | Google response headers, S1 (`http3` section) | Not TLS. A large project; a long-lived session that stays on h2 is a weak signal. |
| TCP/IP | macOS stack | macOS stack | measured | Matches. |

## Options for the TLS gap

1. Stay on rustls, config only. Reaches ML-DSA schemes and ECH GREASE. JA4 still differs in
   all three parts. Little value alone.
2. Patch rustls (a vendored fork via `[patch.crates-io]`). Add GREASE at BoringSSL's
   positions, a hook to emit raw extensions (renegotiation_info instead of the SCSV, SCT,
   trust_anchors, ALPS), and extra cipher suite ids to offer. Then implement the client side of
   ALPS and SCT acceptance. The result can be byte-for-byte Chrome's hello, in Rust. Cost:
   medium to large, plus keeping the fork rebased. The six legacy suites need the CBC suites
   implemented or a clean failure when chosen.
3. BoringSSL through the `boring` crate (what Chrome-impersonating Rust clients use). It sets
   GREASE, extension permutation, ALPS, SCT, OCSP, cert compression and ECH GREASE with
   documented calls, and it tracks Chrome because it is Chrome's TLS library. Cost: a C
   dependency and build toolchain, against the pure-Rust direction of Mars. Maintainer
   decision.
4. Our own TLS 1.3 client. `legacy.rs` already writes a TLS 1.2 hello by hand, but a full
   TLS 1.3 client is the largest option and the riskiest for security. Not recommended.

## Recommended order, cheapest first

1. Honour `Accept-CH` and send Chrome's high-entropy client hints (arch `"arm"`, bitness
   `"64"`, full version list, model `""`, platform version, wow64 `?0`, form factors
   `"Desktop"`), with values that agree with what V9's `navigator.userAgentData` reports. Small,
   own code, and it targets the exact request Google answered with 429. In the same pass, diff
   that second Google request (headers, cookies set by the challenge script) and check the
   reload path sends `cache-control: max-age=0`. Evidence: the follow-up search request
   returns results instead of `/sorry/`, or the 429 persists and points at TLS or the JS
   environment.
2. Decide between option 2 (rustls fork) and option 3 (BoringSSL) with the maintainer, then
   build one to the target JA4 `t13d1517h2_8daaf6152771_cb7bf5808d99` with GREASE. This is the
   change that actually moves TLS-based detection (Cloudflare, Akamai, and likely Google's
   scoring), because hashes only match once everything matches. Inside it, do the rustls-only
   pieces first (ML-DSA schemes, ECH GREASE with a TLS 1.2 retry) since both options need the
   same target list. Evidence: tls.peet.ws through `examples/fetch.rs` shows the target JA4, and
   `stackoverflow.com/questions` (Cloudflare 403 today, per `crates/relay/README.md`) loads.
3. Huffman and incremental indexing in the HPACK encoder, so header block bytes look like
   Chrome's. Small, own code, low detection value; do it after TLS.

Later, if still blocked: HTTP/3, and the Google-only `x-browser-*` headers (maintainer
decision). Also note `crates/relay/README.md` is stale: rustls 0.23.45 does randomize extension
order, and X25519MLKEM768 is now offered first through our own ML-KEM.

## Sources

- [S1] curl-impersonate, Chrome 152 macOS capture:
  https://github.com/lexiforest/curl-impersonate/blob/main/tests/signatures/chrome152_macos.yaml
  (curl_cffi lists `chrome152_macos` as macOS 26.0:
  https://github.com/lexiforest/curl_cffi/blob/main/curl_cffi/fingerprints.py)
- [S2] BoringSSL headers, code points for application_settings (17613), encrypted_client_hello
  (0xfe0d), trust_anchors (0xca34), certificate_timestamp (18), ML-DSA (0x0904 to 0x0906),
  X25519MLKEM768 (0x11ec):
  https://boringssl.googlesource.com/boringssl/+/refs/heads/main/include/openssl/tls1.h and
  `include/openssl/ssl.h`
- [S3] Chromium variations headers (`x-client-data`):
  https://cobalt.googlesource.com/cobalt/+/master%5E/components/variations/net/variations_http_headers.cc
- [S4] Third-party write-ups of `x-browser-validation` (unofficial, unverified):
  https://roundproxies.com/blog/chrome-x-browser-validation-header/ and
  https://feedbagel.com/post/understanding-chromes-x-browser-validation-header-reverse-engineering-and-genera
- tls.peet.ws `/api/all` and `/api/clean` responses to Relay, 2026-10-28 (measured values).
- rustls 0.23.45 source: `src/msgs/handshake.rs` (extension order randomization),
  `src/client/hs.rs` (SCSV, key shares), `src/client/builder.rs` (`with_ech` forces TLS 1.3).
