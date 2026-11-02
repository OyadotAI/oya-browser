# Security review

Establish attacker capabilities, assets, entry points, and trust boundaries from the actual
deployment and changed code. Read the diff through each boundary; do not equate a keyword or
scanner match with an exploit.

- Trace authentication, authorization, object ownership and tenant scope on both reads and
  writes. Exercise expired sessions, stale cookies, cross-tenant IDs, and unauthenticated routes.
- Inspect token lifetimes, cookie flags, rotation, CSRF, CORS, redirects, webhook signatures,
  replay handling, and request/body limits as relevant to the architecture.
- Follow untrusted values into shells, SQL, HTML, filesystem paths, URLs, deserializers, and
  agent instructions. Check symlinks, traversal, option injection, SSRF, and loopback access.
- Distinguish first-party credentials from public examples, test certificates and dependency
  caches. Identify tracked exposure and production usage without printing secret values.
  Recommend rotation for real exposed credentials; require existing authorization before
  changing remote credentials, rewriting history, or deleting material.
- Verify a dependency advisory against the resolved version and actual reachability using
  current primary sources through Oya. Do not invent a CVE or treat a stale version as an exploit.

Use benign scratch reproductions; do not execute untrusted hooks or contact an attacker's URL.
Use `../assets/findings.md` with a reachable attack path, preconditions, impact, evidence, and
repair. Classify uncertain exposure as an investigation lead. Finish with coverage and remaining
unknowns. Keep implementation unchanged unless remediation was explicitly assigned.
