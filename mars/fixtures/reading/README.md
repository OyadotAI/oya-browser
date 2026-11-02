<!-- Provenance of frozen public compatibility inputs, not a browser conformance claim. -->
# Static reading fixtures

`example.html` and `httpbin.html` preserve the exact public response bodies observed
through Oya Native's owned network service on 2026-10-23 from
`https://example.com/` and `https://httpbin.org/html`. They are offline regression
inputs; tests do not contact either service. Preserve their bytes and resource
references rather than rewriting the page into the supported subset.

The httpbin paragraph is an excerpt from Herman Melville's public-domain
*Moby-Dick*. Its complete 3,588 UTF-8 bytes must remain one original text-bearing
node. The example input contains a required `/s.js` reference; stage-level CSS
tests keep it intact. Passing a stage-level test does not establish successful
navigation when that required script uses unsupported DOM/JavaScript features.

`example-s.js` is that script, frozen byte-for-byte: observed 2026-10-23 at
`https://example.com/s.js`, served as `text/javascript; charset=utf-8` with
Content-Length 1963 and no Content-Encoding. It is 1,963 bytes, ends in `')` and a
newline, and has sha256
`535ef2c3425a9d1214bcd1a8d5430d904444135e48a4f113ba1e6f6e29925b0a`. The live script
can change; this copy is the regression gate and live runs are evidence only.

These data fixtures retain their original HTML instead of a repository source
header so the unchanged-source regression remains meaningful.
