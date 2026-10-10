# HIPAA Security Rule, evidence pack

Generated 2026-10-07T13:44:37.127Z by `compliance/run-evidence.mjs`. Regenerate it per release; do not edit by hand.

**7 proven · 3 known gaps · 0 failing**

| § | Control | Verdict | Implementation | What the check found |
|:--|:--|:--|:--|:--|
| 164.312(b) | Audit controls | PASS | `server/src/platform/audit.ts`<br>`server/migrations/004_control_plane.sql` | 27 passed, 0 failed; audit_log carries who/what/outcome/source: true |
| 164.312(c)(1) | Integrity | PASS | `server/src/platform/audit-chain.ts`<br>`server/migrations/012_audit_tamper_evidence.sql` | 17 passed, 0 failed |
| 164.312(c)(2) | Mechanism to authenticate ePHI | PASS | `server/src/platform/audit-chain.ts` | update refused: true; delete refused: true; service_role holds INSERT,SELECT |
| 164.312(d) | Person or entity authentication | PASS | `server/src/modules/challenges/` | 40 passed, 0 failed |
| 164.312(e)(1) | Transmission security | PASS | `server/src/modules/control/egress.ts`<br>`browser/src/main/identity/host-rules.ts` | 11 passed, 0 failed |
| 164.308(a)(4) | Information access management | PASS | `server/src/modules/control/egress.ts` | 11 passed, 0 failed |
| 164.312(a)(2)(iv) | Encryption and decryption | PASS | `server/src/platform/secrets.ts`<br>`server/migrations/010_hash_api_keys.sql` | 19 passed, 0 failed; api keys stored as sha256: true |
| 164.502(b) | Minimum necessary, recordings and logs | GAP | — | redaction exists in 14 files but targets secrets and placeholders, not PHI |
| 164.308(b)(1) | Business associate contracts | GAP | — | no BAA with Google Cloud; no BAA with Supabase; no BAA with Model provider (OpenAI, Anthropic or Vertex AI) |
| 164.316(b)(2) | Documentation retention | GAP | `server/src/modules/control/service/constants.ts`<br>`server/src/modules/control/worker/maintenance.ts` | default audit retention floor is 365 days; six years is 2190 |

## How each verdict was reached

- **164.312(b)**: `node --import ./tests/support/hermetic.js --test tests/unit/platform/audit.test.ts`
- **164.312(c)(1)**: `node --import ./tests/support/hermetic.js --test tests/unit/platform/audit-chain.test.ts`
- **164.312(c)(2)**: `psql: update/delete against audit_log, then read its grants`
- **164.312(d)**: `node --import ./tests/support/hermetic.js --test tests/unit/modules/challenges/mfa.test.ts tests/unit/modules/challenges/totp.test.ts tests/unit/modules/challenges/mfa-factors.test.ts`
- **164.312(e)(1)**: `node --import ./tests/support/hermetic.js --test tests/unit/modules/control/egress.test.ts`
- **164.308(a)(4)**: `node --import ./tests/support/hermetic.js --test tests/unit/modules/control/egress.test.ts`
- **164.312(a)(2)(iv)**: `node --import ./tests/support/hermetic.js --test tests/unit/platform/secrets.test.ts`
- **164.502(b)**: `grep -rl redact server/src`
- **164.308(b)(1)**: `inspect OYA_RESIDENTIAL_PROXY_URL / BENCH_PROXY_URL, and compliance/baas.json`
- **164.316(b)(2)**: `read DEFAULT_AUDIT_RETENTION_FLOOR_DAYS in server/src/modules/control/service/constants.ts`

## Open gaps

### 164.502(b), Minimum necessary, recordings and logs

Redaction covers secrets and form placeholders, not PHI. Screenshots and DOM snapshots of a payer portal are PHI at rest. Closing it needs a redaction pass over recordings and a retention clock per artifact.

### 164.308(b)(1), Business associate contracts

Every hosted subprocessor on the ePHI path needs a signed BAA, recorded with its date in compliance/baas.json. A residential proxy vendor cannot give one, so healthcare deployments must egress directly or through customer-owned infrastructure.

### 164.316(b)(2), Documentation retention

Audit records have an enforced retention floor and expired recordings are purged, but the default floor is one year. HIPAA documentation retention is six years, so the floor must default to 2190 days, or be raised to it for HIPAA projects.

## Scope

This pack covers the Oya browser infrastructure: the control plane, the browser and the
audit trail. It does not cover the customer application driving it, the cloud provider
underneath it, or any workflow content. A covered entity remains responsible for its own
risk analysis under §164.308(a)(1).
