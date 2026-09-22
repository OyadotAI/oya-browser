# Compliance evidence

What is here:

| File | What it is | Who reads it |
|:--|:--|:--|
| `EVIDENCE.md` | Generated. Every HIPAA control, the code implementing it, the check that ran, what it returned. | A customer's auditor or security reviewer |
| `controls.mjs` / `checks.mjs` | The control map and the checks. Editing these is how the pack changes. | Whoever changes a control |

## Regenerating the pack

```bash
./compliance/scratch-db.sh          # throwaway Postgres with 004 + 012 applied
node compliance/run-evidence.mjs    # writes EVIDENCE.md and evidence.json
docker rm -f pg-audit               # when finished
```

It exits non-zero when a control fails outright, so CI can gate a release on it. A control that
is a known gap exits zero and stays visible in the report: the pack is meant to be handed over
with its gaps intact, not to be made green.

The scratch database is deliberate: the append-only proof applies the real migrations to an empty
Postgres and tries to edit history there. It needs no production access, so it runs in CI and on
a laptop, and it cannot be confused with a claim about a live deployment.

## What the pack does not cover

The customer's own application, the cloud underneath, and workflow content. A covered entity still
owes its own risk analysis under §164.308(a)(1). Say so rather than implying the pack is a
certification. It is evidence about this repository at a commit. Outbound analytics is outside it
too: PostHog and the Slack webhooks are operator-configured, off by default, and best effort; the
audit log is the evidence, not them.
