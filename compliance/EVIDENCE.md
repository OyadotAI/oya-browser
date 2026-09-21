# HIPAA Security Rule: evidence pack

Generated 2026-09-20T16:26:12.283Z by `compliance/run-evidence.mjs`. Regenerate it per release; do not edit by hand.

**8 proven · 2 known gaps · 0 failing**

| § | Control | Verdict | Implementation | What the check found |
|:--|:--|:--|:--|:--|
| 164.312(b) | Audit controls | PASS | `server/src/platform/audit.ts`<br>`server/migrations/004_control_plane.sql` | 11 passed, 0 failed; audit_log carries who/what/outcome/source: true |
| 164.312(c)(1) | Integrity | PASS | `server/src/platform/audit-chain.ts`<br>`server/migrations/012_audit_tamper_evidence.sql` | 11 passed, 0 failed |
| 164.312(c)(2) | Mechanism to authenticate ePHI | PASS | `server/src/platform/audit-chain.ts` | update refused: true; delete refused: true; service_role holds INSERT,SELECT |
| 164.312(d) | Person or entity authentication | PASS | `server/src/modules/challenges/` | 36 passed, 0 failed |
| 164.312(e)(1) | Transmission security | PASS | `server/src/modules/control/egress.ts`<br>`browser/governance.js` | 11 passed, 0 failed |
| 164.308(a)(4) | Information access management | PASS | `server/src/modules/control/egress.ts` | 11 passed, 0 failed |
| 164.312(a)(2)(iv) | Encryption and decryption | PASS | `server/src/platform/secrets.ts`<br>`server/migrations/010_hash_api_keys.sql` | 8 passed, 0 failed; api keys stored as sha256: true |
| 164.502(b) | Minimum necessary, recordings and logs | GAP | — | redaction exists in 10 files but targets secrets and placeholders, not PHI |
| 164.308(b)(1) | Business associate contracts | PASS | — | no third-party proxy configured: egress is direct or customer-owned |
| 164.316(b)(2) | Documentation retention | GAP | — | no audit retention floor and no purge job found in server/src |

## How each verdict was reached

- **164.312(b)**: `node --import ./tests/support/hermetic.js --test tests/unit/platform/audit.test.ts`
- **164.312(c)(1)**: `node --import ./tests/support/hermetic.js --test tests/unit/platform/audit-chain.test.ts`
- **164.312(c)(2)**: `psql: update/delete against audit_log, then read its grants`
- **164.312(d)**: `node --import ./tests/support/hermetic.js --test tests/unit/modules/challenges/mfa.test.ts tests/unit/modules/challenges/totp.test.ts tests/unit/modules/challenges/mfa-factors.test.ts`
- **164.312(e)(1)**: `node --import ./tests/support/hermetic.js --test tests/unit/modules/control/egress.test.ts`
- **164.308(a)(4)**: `node --import ./tests/support/hermetic.js --test tests/unit/modules/control/egress.test.ts`
- **164.312(a)(2)(iv)**: `node --import ./tests/support/hermetic.js --test tests/unit/platform/secrets.test.ts`
- **164.502(b)**: `grep -rl redact server/src`
- **164.308(b)(1)**: `inspect OYA_RESIDENTIAL_PROXY_URL / BENCH_PROXY_URL`
- **164.316(b)(2)**: `grep -rEn "AUDIT_RETENTION|RETENTION_YEARS" server/src`

## Open gaps

### 164.502(b): Minimum necessary, recordings and logs

Redaction covers secrets and form placeholders, not PHI. Screenshots and DOM snapshots of a payer portal are PHI at rest. Closing it needs a redaction pass over recordings and a retention clock per artifact.

### 164.316(b)(2): Documentation retention

No retention floor is enforced on the audit trail and no purge job exists for artifacts past their window. Needs a retention policy per artifact class with a six-year floor for audit records.

## Scope

This pack covers the Oya browser infrastructure: the control plane, the browser and the
audit trail. It does not cover the customer application driving it, the cloud provider
underneath it, or any workflow content. A covered entity remains responsible for its own
risk analysis under §164.308(a)(1).
