# Durable control plane

Oya keeps project ownership, session reservations, lifecycle outcomes, control leases, credentials, event delivery, and estimated budgets in durable storage. Existing API keys and browser endpoints remain supported. Each legacy key maps to an isolated project; membership grants access without moving its personas or encrypted profiles to another tenant.

## Run locally

Use Node 22.13 or newer. With no Supabase configuration, the server uses `OYA_DATA_DIR/control.sqlite` (default `server/data/control.sqlite`). Keep this directory on persistent storage. SQLite permits one server process; a PID lock rejects concurrent use. Do not remove a live process's lock.

Set a stable `OYA_PROFILE_SECRET` before creating data. Back up this secret separately from the database. Changing it makes encrypted profiles, queue requests, webhook secrets, and provider cleanup credentials unreadable.

The first startup copies legacy local data into `migration-backup-v1`, records an ownership mapping without raw keys, and marks the migration complete only after import succeeds. Legacy browser inventory becomes disconnected; migration does not assume ownership of an external resource it cannot verify. Existing profile encryption contexts are preserved. Back up the database and local data before upgrading; restore both with the matching application version to roll back.

## Run multiple replicas

1. Apply the existing migrations, followed by `server/migrations/008_durable_control.sql`, to your Supabase Postgres database. The migration is idempotent and touches only the `oya_browser` schema. When the project's migration history belongs to another codebase, apply the file directly instead of with `db push`: `npx supabase db query --linked --file server/migrations/008_durable_control.sql`. Expose the `oya_browser` schema to PostgREST. The control RPCs are executable only by `service_role`; never give that credential to a browser or client.
2. Configure the same `SUPABASE_URL`, `SUPABASE_SERVICE_KEY`, and `OYA_PROFILE_SECRET` on all replicas.
3. Set a distinct, reachable `OYA_INSTANCE_URL` on each replica (for example `http://oya-a:3100`). Set the same strong `OYA_CLUSTER_SECRET` on every replica. Optional `OYA_INSTANCE_ID` must be unique per running process.
4. Configure your load balancer for HTTP, WebSocket upgrades, and unbuffered SSE. Route readiness checks to `/readyz`; `/livez` checks process liveness only.
5. For shared recordings, create a **private** Supabase Storage bucket and set `OYA_RECORDING_BUCKET` on all replicas. Keep each replica's local recording spool persistent until uploads succeed.

State is stored as versioned rows: a transaction writes only the rows it changed, with a compare-and-swap on each row's version, so unrelated sessions and projects never contend. Admission decisions serialize on a per-project lock row; transactions on one replica that lock the same row queue in-process, so under a burst only other replicas contend, and retries back off exponentially with jitter. Events are an append-only log, and webhook deliveries are created in the same transaction as the event that triggers them. Commands use separate per-session gate rows, with owner fencing and in-flight counts. Requests arriving on another replica, including per-browser MCP (`/mcp/{id}`) and live streams, are authenticated and routed to the leased owner; forwarding preserves the caller's scoped credential. Losing storage denies new commands. Losing a browser process does not preserve its JavaScript heap or replay uncertain actions.

Provider configurations and other legacy file-backed settings should be provisioned consistently across replicas. Measured on a local Postgres 16 through the same RPCs, without the PostgREST network hop (4 replicas, 32 concurrent clients): admission across many projects is about 3,800 per second; single-session updates about 7,000 per second; the command gate about 11,000 per second; a worker tick over 3,000 live sessions about 60 ms. Bursts of admissions into one project are the ceiling, at about 60 per second with p99 near 4 seconds, because they serialize on that project's lock row across replicas and each one counts the project's live sessions. Integration coverage is for two replicas, and production figures through PostgREST will be lower.

## Configure governed browsers

Strict governance is available for `oya-selfhosted` browsers created by the managed Docker runtime. External providers and manually attached CDP browsers remain available with limited guarantees; requests requiring managed policy or hard budgets are rejected when the runtime cannot enforce them.

Build the browser image:

```sh
docker build -t oya-browser:managed browser
```

Create an internal Docker bridge network. Put the control service and its egress proxy on that network, with a separate uplink for the trusted control service. Browser containers must have only the internal network. The operator must give the control service access to the Docker daemon; the server image includes the Docker CLI but does not mount a Docker socket automatically.

| Variable | Meaning |
| --- | --- |
| `OYA_MANAGED_NETWORK` | Existing internal Docker bridge network |
| `OYA_MANAGED_IMAGE` | Governance-enabled browser image; its immutable ID is recorded |
| `OYA_MANAGED_CONTROL_URL` | Browser-reachable control WebSocket URL, ending in `/ws` |
| `OYA_MANAGED_PROXY_URL` | Browser-reachable HTTP egress proxy URL |
| `OYA_MANAGED_REGION` | Operator-declared runtime region, default `local` |
| `OYA_EGRESS_PORT` | Enables the authenticated egress listener |
| `OYA_EGRESS_HOST` | Listener address, default `127.0.0.1`; use a reachable interface in the isolated network |

Creation verifies the network is internal and the image carries the governance label. Session-specific enrollment and proxy credentials bind the container to its reservation. The container never receives the project key: its API credential can register only its own session's browser, is refused by every other API, and stops working when the session ends. Browser hooks enforce navigation and subresource policy; the proxy pins public DNS results and denies private destinations and nonstandard ports. Container creation drops capabilities and sets resource limits. Cleanup checks ownership labels and the original Docker daemon ID before deleting anything. A replica that cannot reach that daemon retains the cleanup obligation.

Policy fields are `allowedHosts`, `humanHosts`, `region`, and `redactRecording`. Host rules use lowercase names, `*.example.com` (which excludes the bare domain), or a `*` inside a label, such as `*-aiplatform.googleapis.com` for regional Google endpoints. A wildcard never crosses a label boundary. Prefix wildcards match every sibling in that position, so `*-app.example.com` grants any tenant of that zone — use them only where the labels are not third-party registerable. A policy takes at most 100 rules, each at most 253 characters with at most three wildcards. Project and session policies are intersected. `humanHosts` requires human control for matching requests; both the browser hooks and the egress proxy enforce it. `redactRecording: true` disables recording rather than storing potentially sensitive frames. Policies are fixed for an admitted session; project changes apply at future admission, including queued work. Region is an operator assertion, not independent geographic attestation.

## Create, queue, stop, and recover

```sh
curl -X POST http://localhost:3100/api/browsers/start \
  -H "Authorization: Bearer $OYA_API_KEY" \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: checkout-42' \
  -d '{"provider":"oya-selfhosted","governed":true,"queueMs":30000,"priority":"normal","policy":{"allowedHosts":["example.com"]}}'
```

A reservation precedes provider creation. Reusing an idempotency key with the same body returns the recorded operation; changing the body returns a conflict. Keys are retained for seven days. A queued response is HTTP 202 with an operation/session ID. Poll `/api/control/sessions/{id}`. Queuing is opt-in, at most five minutes, and honors priority and project fairness. Cancellation does not free capacity until cleanup is confirmed.

States include `queued`, `provisioning`, `ready`, `disconnected`, `cleanup_pending`, `unknown_outcome`, `stopped`, and `failed`. Unresolved resources remain visible and count against admission. Cleanup retries with backoff. Unknown provider outcomes without a usable deletion descriptor require operator reconciliation: verify the resource is gone, then call `/stop` with `{ "force": true }`, which records `session.stopped` with reason `reconciled`.

Attach-only sessions (`cdp`, `oya-desktop`, gateway) have no resource of Oya's to delete, so a failed attach is `failed`, not `unknown_outcome`. A lost CDP or gateway attachment is `stopped`. A dial-in browser that loses its connection is `disconnected`; it does not hold capacity, and reconnecting rechecks project and persona limits. A provisioning browser may enroll on any replica. Managed browsers are bound to their reservation by the enrollment token. Terminal sessions are pruned seven days after their last event. Estimated spend accrues on the project (`costUsd`), so pruning loses none of it, and the event log keeps their history.

`POST /api/control/sessions/{id}/stop` saves profile state before cleanup. `{ "force": true }` permits cleanup despite a profile-save error. Use `/cancel` for queued work. Recovery returns the original ready session when available. Otherwise `/recover` requires explicit `{ "replace": true }`, and only after the original is stopped or failed. For a plain CDP replacement, supply the new `wsUrl` explicitly. Replacement preserves admitted policies and restores supported profile data; it cannot promise that the previous action did or did not run. Never automatically replay payments or other non-idempotent browser actions after `command_outcome_unknown`.

## Access and human control

Administrators manage project settings, credentials, members, and webhooks. Operators operate sessions. Viewers read sanitized inventory and events, without CDP tickets, recordings, or live input. Credentials are hashed and shown once. Rotate by creating a replacement, updating clients, then revoking the old ID. Revocation is checked on requests and periodically on active attachments. A validation outage closes attachments rather than retaining access.

Administrators create one-use, seven-day invitation codes. Users join with their account JWT through `/api/auth/projects/join`; `/api/auth/projects/{id}/access` issues a scoped, one-hour console credential. Removing a member invalidates their member-linked credentials. Existing owner keys continue to work for compatibility.

CDP and live-stream URLs use one-use, session-bound tickets valid for 60 seconds. Tickets preserve the underlying credential's role and revocation. Obtain a fresh ticket for reconnects. Legacy credential URLs remain accepted unless `OYA_ALLOW_LEGACY_QUERY_KEYS=false` is configured.

Takeover uses `/api/control/sessions/{id}/control` with `acquire`, `release`, or `resume`. Acquisition waits up to ten seconds for in-flight commands to settle and grants a five-minute lease tied to the caller's credential. A CDP command the browser has not answered within 60 seconds (`OYA_STUCK_COMMAND_MS`) stops blocking takeover; its outcome is unknown. Human input goes through `/input`. Release or expiry leaves the agent paused; resumption is explicit. Raw CDP agent commands also obey this gate.

## Budgets, audit, and recordings

Project settings include `maxConcurrent`, `budgetUsd`, provider `rates` in USD/browser-hour, `recordingDays` (default 7), and `auditDays` (default 90). Budgets require configured rates and managed provisioning. Admission reserves one minute of estimated cost; metering accrues every 30 seconds into the session and the project's `costUsd`, and requests cleanup before estimated spend reaches the configured limit. Provider billing, cleanup delays, and outages can produce differences from these estimates. These controls are not a provider invoice guarantee.

The `/api/control` overview includes the latest 100 events. Read the full retained log using `/api/control/events?after={cursor}` or export administrator audit data from `/api/control/audit/export` as NDJSON. Webhooks use a durable outbox, at-least-once delivery, exponential retries for 24 hours, and manual replay through `/api/control/deliveries/{id}/replay`. Deduplicate with `Oya-Event-Id`. Validate `Oya-Signature: t=<seconds>,v1=<hex>` as HMAC-SHA256 of `<seconds>.<raw request body>` with the returned secret, using a constant-time comparison and an appropriate timestamp tolerance.

Recording access stays tenant-scoped. Private object storage makes completed recordings available across replicas. Failed uploads remain in the local spool for retry. Retention deletes local and archived recordings; pending upload spools require persistent disk. Real Supabase Storage integration should be verified in the deployment's private bucket before relying on archival durability.

## Clients and validation

The console's Control → Project operations view exposes inventory, takeover, limits, policies, credentials, membership invitations, event delivery, and activity. The TypeScript SDK exposes these operations under `oya.control`. CLI equivalents include `oya control`, `oya sessions`, `oya cancel`, `oya recover --replace`, `oya takeover`, `oya release`, `oya resume`, `oya members`, `oya credential`, and `oya webhook`. Run `oya help` for argument forms.

```sh
npm test
npm run build:sdk
npm run lint --prefix ui
npm run build --prefix ui -- --webpack
# Live Supabase smoke test through PostgREST; deletes everything it creates:
node --env-file=server/.env server/test-control-supabase.js
# Optional isolated Postgres fixture with migration 008 applied:
PGHOST=/path/to/socket PGPORT=55439 node server/test-control-postgres.js
# Real Docker/Electron contract:
OYA_TEST_IMAGE=oya-browser:managed node server/test-managed-runtime.js
```

The normal suite covers local persistence, concurrent admission, idempotency, role boundaries, control leases, revocation, and cross-replica HTTP/CDP routing alongside existing provider and browser tests. Docker and Postgres contracts are separate so ordinary tests do not require those services.
