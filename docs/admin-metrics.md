# Staff metrics and reporting scope

Admin has five domains: Overview, Customers, Self-hosted, Keel and Fleet.
Each source refreshes independently. Its last successful snapshot remains visible
on ordinary refresh failure, with a stale-data notice. Authentication or staff
permission refusals clear protected content. Replacing a credential discards prior
requests, selected customer state and one-time issued keys.

## Time and populations

Overview comparisons and charts use seven completed UTC dates against the preceding
seven. Daily detail covers thirty completed dates ending yesterday. Weekly active
accounts are distinct identities across the week, not sums of daily active counts.
Unowned API-key activity cannot become an invented account.

Customer lookup uses that customer's allowance period and reports its boundary and
snapshot time. Existing accounting stores hourly buckets, so boundary hours are not
precise sub-hour measurements. Calendar-month top-ten rankings have a separate,
explicit start and snapshot boundary.

Self-hosted headline counts include every recorded install, before the most-recent
fifty-row table limit. Reporting means a server receipt in the rolling seven-day
window, not currently online. The above-cap signal includes only recently reporting
installs with no reported license ID. Reported IDs are observations; issued Oya
licenses are a separate authenticated inventory.

Fleet describes the responding server's connection registry, including its
Oya Cloud + self-hosted subtotal. It is not a Kubernetes-wide global inventory.

## Finance

`GET /api/admin/revenue` is staff guarded and independent of core activity.
Active fixed-price MRR includes active licensed per-unit month/year prices normalized
by interval count. Metered prices are excluded. This is not Stripe's configurable
analytics MRR. Paid invoice totals use `amount_paid` and the final `paid_at` date;
old-created invoices paid recently are included. This is not an individual cash
receipt or refund ledger. Amounts remain separate by currency and carry verified API
minor-unit exponents, including Stripe's ISK/UGX/HUF/TWD charge-unit rules.

Each metric owns a ten-second abortable deadline, twenty-request budget including
nested item pages, and two-thousand-object budget. No automatic retries. Both metrics
can run concurrently, but a failed one cannot erase the other. Partial pagination,
missing dates/quantities, unsupported tiers/intervals, unverified currency units or
unsafe arithmetic yield `incomplete` with null amounts. Upstream failures yield
`unavailable`; absent configuration yields `not_configured`. Only a complete empty
collection means measured zero. Private overview clients must request `?version=2`;
older clients receive a reload-required conflict rather than misleading legacy money.

## Keel

`GET /api/admin/keel` uses the same staff/MFA guards. Installs and their latest nullable
reported license IDs use rolling seven-day server receipts, independently of immutable
usage dates. License IDs are not full signed keys and do not establish issued-key
inventory or validity. Tables show at most fifty rows; headline populations are complete.

Usage covers thirty completed UTC dates. Agent-hours sum supervised top-level run
intervals, including tools and approval waits, while excluding idle persistent sessions,
followers, verification and autocommit. Concurrent lanes add. Projects count distinct
(installation, opaque repository) identities where a supervised run started in the
period. Worktrees on one installation deduplicate; the same repository on two installs
counts twice. No repository path, name, remote, prompt or transcript is reported.

New senders use schema 2 with up to seven immutable daily snapshots per request,
independently nullable runtime/project measurements, and an empty report list for presence.
Old six-field senders remain accepted. New measurements begin the next full UTC day after
upgrade. Legacy measurements remain null; independently measured values fill a missing
measurement once without overwriting original steps or receipt time. Coverage is shown
beside totals. Old releases can undercount steps. The sender checkpoints monotonic runtime,
splits UTC boundaries, and persists a bounded ninety-day retry backlog; accepted snapshots
are removed only after the receiver acknowledges them. Expiration emits a count-only diagnostic.

Apply additive migration `026_keel_activity.sql` through the normal runner before the
updated hosted server. It backfills original receipt times without creating activity.
SQLite adds nullable columns/tables; file and SQLite drivers perform equivalent idempotent
presence backfill. Roll out the receiver before the new Keel sender. A sender release needs
the reviewed private dependency published at a real immutable Git SHA, both Keel lockfiles
regenerated, and the normal build gate run without a local dependency override.
