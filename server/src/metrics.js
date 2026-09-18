/**
 * In-process metrics. No dependencies, no scrape-time work beyond formatting.
 *
 * Cardinality rule: never label by api key, browser id, or URL. At 1k-5k
 * browsers those become 5000-way label explosions that cost more than the
 * thing being measured. Per-key numbers live in usage.js, which is keyed
 * storage rather than a time series.
 */

import { monitorEventLoopDelay } from 'perf_hooks';

const registry = new Map();

const labelKey = (labels) => {
  const keys = Object.keys(labels).sort();
  if (!keys.length) return '';
  return keys.map((k) => `${k}=${labels[k]}`).join(',');
};

const escapeValue = (v) => String(v).replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\n/g, '\\n');

const renderLabels = (labels) => {
  const keys = Object.keys(labels).sort();
  if (!keys.length) return '';
  return `{${keys.map((k) => `${k}="${escapeValue(labels[k])}"`).join(',')}}`;
};

class Metric {
  constructor(name, help, type) {
    Object.assign(this, { name, help, type });
    this.series = new Map(); // labelKey -> { labels, value | histogram state }
  }
  entry(labels) {
    const key = labelKey(labels);
    let s = this.series.get(key);
    if (!s) {
      // A runaway label set is a bug; drop rather than grow without bound.
      if (this.series.size >= 2000) return null;
      s = this.init(labels);
      this.series.set(key, s);
    }
    return s;
  }
}

class Counter extends Metric {
  constructor(name, help) { super(name, help, 'counter'); }
  init(labels) { return { labels, value: 0 }; }
  inc(labels = {}, by = 1) { const e = this.entry(labels); if (e) e.value += by; }
  render(out) {
    for (const s of this.series.values()) out.push(`${this.name}${renderLabels(s.labels)} ${s.value}`);
  }
}

class Gauge extends Metric {
  constructor(name, help, collect) { super(name, help, 'gauge'); this.collect = collect; }
  init(labels) { return { labels, value: 0 }; }
  set(labels = {}, value) { const e = this.entry(labels); if (e) e.value = value; }
  inc(labels = {}, by = 1) { const e = this.entry(labels); if (e) e.value += by; }
  dec(labels = {}, by = 1) { this.inc(labels, -by); }
  render(out) {
    if (this.collect) { const v = this.collect(); if (Number.isFinite(v)) this.set({}, v); }
    for (const s of this.series.values()) out.push(`${this.name}${renderLabels(s.labels)} ${s.value}`);
  }
}

// Fixed buckets in milliseconds. Wide, because a navigate legitimately takes
// tens of seconds while a frame capture takes single-digit ms.
const BUCKETS = [5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000, 30000, 60000];

class Histogram extends Metric {
  constructor(name, help) { super(name, help, 'histogram'); }
  init(labels) { return { labels, counts: new Array(BUCKETS.length).fill(0), sum: 0, count: 0 }; }
  observe(labels = {}, value) {
    if (!Number.isFinite(value)) return;
    const e = this.entry(labels);
    if (!e) return;
    e.sum += value; e.count += 1;
    for (let i = 0; i < BUCKETS.length; i++) if (value <= BUCKETS[i]) { e.counts[i] += 1; break; }
  }
  render(out) {
    for (const s of this.series.values()) {
      let cumulative = 0;
      for (let i = 0; i < BUCKETS.length; i++) {
        cumulative += s.counts[i];
        out.push(`${this.name}_bucket${renderLabels({ ...s.labels, le: String(BUCKETS[i]) })} ${cumulative}`);
      }
      out.push(`${this.name}_bucket${renderLabels({ ...s.labels, le: '+Inf' })} ${s.count}`);
      out.push(`${this.name}_sum${renderLabels(s.labels)} ${s.sum}`);
      out.push(`${this.name}_count${renderLabels(s.labels)} ${s.count}`);
    }
  }
  /** Approximate percentile from bucket counts — good enough for alerting. */
  quantile(labels, q) {
    const s = this.series.get(labelKey(labels));
    if (!s || !s.count) return null;
    const target = s.count * q;
    let cumulative = 0;
    for (let i = 0; i < BUCKETS.length; i++) {
      cumulative += s.counts[i];
      if (cumulative >= target) return BUCKETS[i];
    }
    return null;
  }
}

function define(Type, name, help, ...rest) {
  if (registry.has(name)) return registry.get(name);
  const m = new Type(name, help, ...rest);
  registry.set(name, m);
  return m;
}

const counter = (name, help) => define(Counter, name, help);
const gauge = (name, help, collect) => define(Gauge, name, help, collect);
const histogram = (name, help) => define(Histogram, name, help);

// ── Event loop health ──
// The single most useful signal for "is this control plane keeping up".
const loopDelay = monitorEventLoopDelay({ resolution: 10 });
loopDelay.enable();

export const metrics = {
  controlCleanup: gauge('oya_control_cleanup_pending', 'Resources awaiting confirmed deletion'),
  controlCleanupAge: gauge('oya_control_cleanup_oldest_seconds', 'Age of oldest pending deletion'),
  controlWebhooks: gauge('oya_control_webhook_pending', 'Event deliveries awaiting acknowledgement'),
  controlQueue: gauge('oya_control_queue_depth', 'Durable queued browser requests'),
  // Connections
  wsConnections: counter('oya_ws_connections_total', 'WebSocket connection attempts by outcome'),
  wsDisconnections: counter('oya_ws_disconnections_total', 'WebSocket disconnections by reason'),
  browsersConnected: gauge('oya_browsers_connected', 'Currently connected browsers'),

  // Commands
  commands: counter('oya_commands_total', 'Browser commands by action and outcome'),
  commandDuration: histogram('oya_command_duration_ms', 'Browser command round-trip time'),
  pendingCommands: gauge('oya_pending_commands', 'Commands awaiting a result'),

  // Chat
  chatRequests: counter('oya_chat_requests_total', 'Chat requests by outcome'),
  chatTokens: counter('oya_chat_tokens_total', 'Model tokens by direction'),
  chatIterations: histogram('oya_chat_iterations', 'Agentic loop iterations per chat request'),

  // Cookies
  cookiePulls: counter('oya_cookie_pulls_total', 'Cookie pull requests served'),
  cookieChanges: counter('oya_cookie_changes_total', 'Cookie changes recorded'),

  // Streaming
  frames: counter('oya_frames_total', 'Frames received from browsers'),
  streamViewers: gauge('oya_stream_viewers', 'Active live-view subscribers'),

  // Provisioning
  sandboxes: counter('oya_sandboxes_total', 'Sandbox operations by op and outcome'),

  // Routing
  routingAcquired: counter('oya_routing_acquired_total', 'Sessions routed to a provider'),
  routingQueued: counter('oya_routing_queued_total', 'Sessions that had to wait for a free slot'),
  routingRejected: counter('oya_routing_rejected_total', 'Sessions rejected by the router'),
  providerFailures: counter('oya_provider_failures_total', 'Provider connect failures'),

  // Gateway
  gatewayConnects: counter('oya_gateway_connects_total', 'CDP gateway connection attempts by outcome'),
  gatewaySessions: gauge('oya_gateway_sessions', 'Live gateway sessions'),
  gatewaySessionDuration: histogram('oya_gateway_session_duration_ms', 'Gateway session lifetime'),
  recordings: counter('oya_recordings_total', 'Session recordings started and stopped'),
  recordedFrames: counter('oya_recorded_frames_total', 'Frames written to session recordings'),

  // Personas
  personasActive: gauge('oya_personas_active_browsers', 'Browsers currently running as some persona'),
  personaCapped: counter('oya_persona_capped_total', 'Starts refused by a persona concurrency cap'),

  // Proxies
  proxyFailures: counter('oya_proxy_failures_total', 'Proxy health check failures'),
  proxyIncoherent: counter('oya_proxy_incoherent_total', 'Personas whose timezone contradicts their exit country'),

  // Challenges
  captchaSeen: counter('oya_captcha_seen_total', 'CAPTCHA challenges detected by type'),
  captchaSolved: counter('oya_captcha_solved_total', 'CAPTCHA solve attempts by type and outcome'),
  mfaCompleted: counter('oya_mfa_completed_total', 'MFA challenges completed by method and outcome'),
  loginCompleted: counter('oya_login_completed_total', 'Sign-ins attempted by method and outcome'),

  // Enforcement
  rateLimited: counter('oya_rate_limited_total', 'Requests rejected by a rate limit'),
  quotaExceeded: counter('oya_quota_exceeded_total', 'Requests rejected by a quota'),

  // HTTP
  httpRequests: counter('oya_http_requests_total', 'HTTP requests by route and status class'),
  httpDuration: histogram('oya_http_duration_ms', 'HTTP request duration'),

  // Audit
  auditEvents: counter('oya_audit_events_total', 'Audit events recorded by action'),

  // Process
  rss: gauge('oya_process_rss_bytes', 'Resident set size', () => process.memoryUsage().rss),
  heapUsed: gauge('oya_process_heap_used_bytes', 'Heap in use', () => process.memoryUsage().heapUsed),
  uptime: gauge('oya_process_uptime_seconds', 'Process uptime', () => process.uptime()),
  loopLagP99: gauge('oya_event_loop_lag_p99_ms', 'Event loop delay, 99th percentile', () => loopDelay.percentile(99) / 1e6),
  loopLagMean: gauge('oya_event_loop_lag_mean_ms', 'Event loop delay, mean', () => loopDelay.mean / 1e6),
};

/** Prometheus text exposition. */
export function render() {
  const out = [];
  for (const m of registry.values()) {
    const body = [];
    m.render(body);
    if (!body.length) continue;
    out.push(`# HELP ${m.name} ${m.help}`, `# TYPE ${m.name} ${m.type}`, ...body);
  }
  return out.join('\n') + '\n';
}

/** Same numbers as JSON, for the dashboard. */
export function snapshot() {
  const out = {};
  for (const m of registry.values()) {
    if (m instanceof Histogram) {
      out[m.name] = [...m.series.values()].map((s) => ({
        labels: s.labels, count: s.count, sum: s.sum,
        avg: s.count ? s.sum / s.count : 0,
        p50: m.quantile(s.labels, 0.5), p95: m.quantile(s.labels, 0.95), p99: m.quantile(s.labels, 0.99),
      }));
    } else {
      if (m.collect) { const v = m.collect(); if (Number.isFinite(v)) m.set({}, v); }
      out[m.name] = [...m.series.values()].map((s) => ({ labels: s.labels, value: s.value }));
    }
  }
  return out;
}

/** Test hook. */
export function reset() {
  for (const m of registry.values()) m.series.clear();
}
