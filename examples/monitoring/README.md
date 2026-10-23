# 📈 Monitoring your Oya project

Everything here reads your project with your own API key. Nothing changes anything.

| You want to know                              | Call                       | Script                                           |
| :-------------------------------------------- | :------------------------- | :----------------------------------------------- |
| Is Oya up, and which version                  | `GET /api/health` (no key) |                                                  |
| Your browsers, their health, the routing pool | `GET /api/fleet`           |                                                  |
| Sessions, their state and cost, recent events | `GET /api/control`         | [`01-status.ts`](01-status.ts)                   |
| This hour's usage and the allowance left      | `GET /api/usage`           | [`01-status.ts`](01-status.ts)                   |
| Every lifecycle event, as it happens          | `GET /api/control/events`  | [`02-events.ts`](02-events.ts)                   |
| Push instead of poll                          | webhooks                   | [`../sdk/08-webhooks.ts`](../sdk/08-webhooks.ts) |
| Prometheus metrics (self-hosted servers)      | `GET /api/metrics`         | [`prometheus/`](prometheus/)                     |

## ⚡ Run the scripts

From the `examples/` folder, with `OYA_API_KEY` in `.env` (see [the setup](../README.md#-setup-in-60-seconds)):

```bash
npx tsx --env-file=.env monitoring/01-status.ts   # one snapshot
npx tsx --env-file=.env monitoring/02-events.ts   # live event tail; Ctrl+C stops
```

`01-status.ts` prints something like:

```
project My project: { stopped: 126, disconnected: 99, ready: 2 }
  oya-755cd6154bb56b72  ready        oya-desktop  persona=p-4f…  $0
this hour: 12 commands (0 errors), 4 browsers started
browsers connected: 2 of 5000
commands left in this window: 120
```

## 🧪 The same calls with curl

```bash
export OYA_API_KEY=...                  # from the dashboard at https://oyabrowser.com
export OYA=https://oyabrowser.com/api   # or your self-hosted server's /api
```

**Is it up?** No key needed.

```bash
curl -s $OYA/health | jq
# { "status": "ok", "version": "1.0.205", "browsers": 2, "uptime": 81234.5 }
```

**Your fleet:** browsers by health, client and persona, open sessions, and the routing pool.

```bash
curl -s $OYA/fleet -H "Authorization: Bearer $OYA_API_KEY" \
  | jq '{browsers: .browsers.byHealth, sessions, queue: .routing.queueDepth, capacity: .routing.capacity}'
```

**Sessions:** each one's state, provider, persona and cost. A session goes `pending` → `provisioning` → `ready`, and ends `stopped`, `cancelled`, `disconnected` or `unknown_outcome`.

```bash
# How many in each state
curl -s $OYA/control/sessions -H "Authorization: Bearer $OYA_API_KEY" \
  | jq 'group_by(.state) | map({state: .[0].state, count: length})'

# Only the live ones
curl -s $OYA/control/sessions -H "Authorization: Bearer $OYA_API_KEY" \
  | jq '.[] | select(.state == "pending" or .state == "provisioning" or .state == "ready") | {id, state, provider, persona, costUsd}'

# One session
curl -s $OYA/control/sessions/$SESSION_ID -H "Authorization: Bearer $OYA_API_KEY" | jq
```

**Usage and limits:** this hour's counters, connected browsers against your quota, and what is left of each rate limit.

```bash
curl -s $OYA/usage -H "Authorization: Bearer $OYA_API_KEY" \
  | jq '{hour: .current, browsers, commandsLeft: .limits.command.remaining}'

# The last 24 hours, hour by hour
curl -s "$OYA/usage?hours=24" -H "Authorization: Bearer $OYA_API_KEY" | jq '.history'
```

**Events:** the durable lifecycle log, 500 at a time after a cursor. Keep the `cursor` from each answer and pass it back as `after`.

```bash
curl -s "$OYA/control/events?after=0" -H "Authorization: Bearer $OYA_API_KEY" \
  | jq '{cursor, events: [.events[] | {id, type, sessionId, at: (.at / 1000 | todate)}]}'
```

A minimal poller in plain shell:

```bash
cursor=0
while true; do
  page=$(curl -s "$OYA/control/events?after=$cursor" -H "Authorization: Bearer $OYA_API_KEY")
  echo "$page" | jq -r '.events[] | "\(.at / 1000 | todate)  \(.type)  \(.sessionId // "-")"'
  cursor=$(echo "$page" | jq '.cursor')
  sleep 5
done
```

Administrators of a project can also export the whole log as NDJSON:

```bash
curl -s $OYA/control/audit/export -H "Authorization: Bearer $OYA_API_KEY" > oya-events.ndjson
```

## 📊 Prometheus (self-hosted servers)

A self-hosted Oya server serves Prometheus metrics at `/api/metrics`. Set `OYA_METRICS_TOKEN` on the server: that token opens `/api/metrics` and nothing else, so a scraper never holds a key that can change anything.

```bash
curl -s https://oya.example.internal/api/metrics -H "Authorization: Bearer $OYA_METRICS_TOKEN" | grep oya_control
```

[`prometheus/prometheus.yml`](prometheus/prometheus.yml) is a scrape config, and [`prometheus/oya-alerts.yml`](prometheus/oya-alerts.yml) has alert rules. The control-plane series:

| Metric                                          | What it is                                |
| :---------------------------------------------- | :---------------------------------------- |
| `oya_control_queue_depth`                       | Browser requests waiting for capacity     |
| `oya_control_cleanup_pending`                   | Browsers waiting for a confirmed deletion |
| `oya_control_cleanup_oldest_seconds`            | How long the oldest one has waited        |
| `oya_control_webhook_pending`                   | Webhook deliveries not yet acknowledged   |
| `oya_routing_acquired_total`                    | Browsers handed out by the routing pool   |
| `oya_routing_queued_total`                      | Requests that had to queue for a browser  |
| `oya_routing_rejected_total`                    | Requests turned away for lack of capacity |
| `oya_browsers_connected`                        | Browsers connected now                    |
| `oya_pending_commands`                          | Commands sent and not yet answered        |
| `oya_commands_total`, `oya_command_duration_ms` | Commands run, and how long they take      |
| `oya_event_loop_lag_mean_ms`                    | Server load: event loop delay             |

## 🔐 Errors

| Status | Means                                                                                                                  |
| :----- | :--------------------------------------------------------------------------------------------------------------------- |
| `401`  | No key, or not a valid one. Send `Authorization: Bearer <key>`.                                                        |
| `403`  | The key is valid but this call needs more: an administrator of the project, or (for `/api/metrics`) the metrics token. |
| `429`  | Rate limited. `GET /api/usage` shows what is left; wait and retry.                                                     |
