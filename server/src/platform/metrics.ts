/**
 * In-process metrics. No dependencies, no scrape-time work beyond formatting.
 *
 * Cardinality rule: never label by api key, browser id, or URL. At 1k-5k
 * browsers those become 5000-way label explosions that cost more than the
 * thing being measured. Per-key numbers live in usage.js, which is keyed
 * storage rather than a time series.
 */

import { monitorEventLoopDelay } from 'perf_hooks';
import {
  HISTOGRAM_BUCKETS as BUCKETS,
  LOOP_LAG_PERCENTILE,
  MAX_SERIES_PER_METRIC,
  NS_PER_MS,
  P50,
  P95,
  P99,
} from './constants.ts';

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

/** One named metric and its series, one per label set. */
abstract class Metric {
  /** Prometheus metric name. */
  declare name: string;
  /** The HELP line. */
  declare help: string;
  /** The TYPE line: counter, gauge or histogram. */
  declare type: string;
  /** The starting state for a new label set. */
  abstract init(labels: object): any;
  /** Series by label key: { labels, value } or histogram state. */
  declare series: Map<any, any>;
  constructor(name, help, type) {
    Object.assign(this, { name, help, type });
    this.series = new Map(); // labelKey -> { labels, value | histogram state }
  }
  /** The series for these labels, created on first use; null once the metric holds MAX_SERIES_PER_METRIC series. */
  entry(labels) {
    const key = labelKey(labels);
    let s = this.series.get(key);
    if (!s) {
      // A runaway label set is a bug; drop rather than grow without bound.
      if (this.series.size >= MAX_SERIES_PER_METRIC) return null;
      s = this.init(labels);
      this.series.set(key, s);
    }
    return s;
  }
}

/** A value that only goes up. */
class Counter extends Metric {
  constructor(name, help) {
    super(name, help, 'counter');
  }
  /** Starts at zero. */
  init(labels) {
    return { labels, value: 0 };
  }
  /** Add to the series for these labels. */
  inc(labels = {}, by = 1) {
    const e = this.entry(labels);
    if (e) e.value += by;
  }
  /** Append one exposition line per series. */
  render(out) {
    for (const s of this.series.values()) out.push(`${this.name}${renderLabels(s.labels)} ${s.value}`);
  }
}

/** A value that goes up and down, optionally read from `collect` at scrape time. */
class Gauge extends Metric {
  /** Reads the current value when rendered; for gauges that sample the process rather than being set. */
  declare collect: any;
  constructor(name, help, collect) {
    super(name, help, 'gauge');
    this.collect = collect;
  }
  /** Starts at zero. */
  init(labels) {
    return { labels, value: 0 };
  }
  /** Set the series for these labels. */
  set(labels = {}, value) {
    const e = this.entry(labels);
    if (e) e.value = value;
  }
  /** Add to the series for these labels. */
  inc(labels = {}, by = 1) {
    const e = this.entry(labels);
    if (e) e.value += by;
  }
  /** Subtract from the series for these labels. */
  dec(labels = {}, by = 1) {
    this.inc(labels, -by);
  }
  /** Read the current value from `collect`, when this gauge samples the process. */
  refresh() {
    if (!this.collect) return;
    const v = this.collect();
    if (Number.isFinite(v)) this.set({}, v);
  }
  /** Refresh from `collect`, then append one exposition line per series. */
  render(out) {
    this.refresh();
    for (const s of this.series.values()) out.push(`${this.name}${renderLabels(s.labels)} ${s.value}`);
  }
}

/** A distribution over the fixed BUCKETS, with sum and count. */
class Histogram extends Metric {
  constructor(name, help) {
    super(name, help, 'histogram');
  }
  /** Empty buckets. */
  init(labels) {
    return { labels, counts: new Array(BUCKETS.length).fill(0), sum: 0, count: 0 };
  }
  /** Record one value; non-finite values are ignored. */
  observe(labels = {}, value) {
    if (!Number.isFinite(value)) return;
    const e = this.entry(labels);
    if (!e) return;
    e.sum += value;
    e.count += 1;
    const i = BUCKETS.findIndex((bound) => value <= bound);
    if (i >= 0) e.counts[i] += 1;
  }
  /** Append cumulative bucket, sum and count lines per series. */
  render(out) {
    for (const s of this.series.values()) this.renderSeries(s, out);
  }
  /** One series' cumulative bucket lines, then its sum and count. */
  renderSeries(s, out) {
    let cumulative = 0;
    for (let i = 0; i < BUCKETS.length; i++) {
      cumulative += s.counts[i];
      out.push(`${this.name}_bucket${renderLabels({ ...s.labels, le: String(BUCKETS[i]) })} ${cumulative}`);
    }
    out.push(`${this.name}_bucket${renderLabels({ ...s.labels, le: '+Inf' })} ${s.count}`);
    out.push(`${this.name}_sum${renderLabels(s.labels)} ${s.sum}`);
    out.push(`${this.name}_count${renderLabels(s.labels)} ${s.count}`);
  }
  /** Approximate percentile from bucket counts, good enough for alerting. */
  quantile(labels, q) {
    const s = this.series.get(labelKey(labels));
    if (!s || !s.count) return null;
    let cumulative = 0;
    for (let i = 0; i < BUCKETS.length; i++) {
      cumulative += s.counts[i];
      if (cumulative >= s.count * q) return BUCKETS[i];
    }
    return null;
  }
}

/** Register a metric once by name; defining it again returns the existing one. */
function define(Type, name, help, ...rest) {
  if (registry.has(name)) return registry.get(name);
  const m = new Type(name, help, ...rest);
  registry.set(name, m);
  return m;
}

const counter = (name, help) => define(Counter, name, help);
const gauge = (name, help, collect?) => define(Gauge, name, help, collect);
const histogram = (name, help) => define(Histogram, name, help);

// ── Event loop health ──
// The single most useful signal for "is this control plane keeping up".
const loopDelay = monitorEventLoopDelay({ resolution: 10 });
loopDelay.enable();

/** Every metric the server records. */
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
  loopLagP99: gauge(
    'oya_event_loop_lag_p99_ms',
    'Event loop delay, 99th percentile',
    () => loopDelay.percentile(LOOP_LAG_PERCENTILE) / NS_PER_MS,
  ),
  loopLagMean: gauge('oya_event_loop_lag_mean_ms', 'Event loop delay, mean', () => loopDelay.mean / NS_PER_MS),
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
  for (const m of registry.values()) out[m.name] = m instanceof Histogram ? histogramSnapshot(m) : valueSnapshot(m);
  return out;
}

/** A histogram's series with their average and approximate percentiles. */
function histogramSnapshot(m: Histogram) {
  return [...m.series.values()].map((s) => ({
    labels: s.labels,
    count: s.count,
    sum: s.sum,
    avg: s.count ? s.sum / s.count : 0,
    ...percentiles(m, s.labels),
  }));
}

/** p50, p95 and p99 of one histogram series. */
function percentiles(m: Histogram, labels) {
  return { p50: m.quantile(labels, P50), p95: m.quantile(labels, P95), p99: m.quantile(labels, P99) };
}

/** A counter's or gauge's series; a sampling gauge is read first. */
function valueSnapshot(m) {
  if (m instanceof Gauge) m.refresh();
  return [...m.series.values()].map((s) => ({ labels: s.labels, value: s.value }));
}

/** Test hook. */
export function reset() {
  for (const m of registry.values()) m.series.clear();
}
