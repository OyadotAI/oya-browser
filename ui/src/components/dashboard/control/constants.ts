/**
 * Every number and fixed list the Control tab runs on, by name: poll rates,
 * display thresholds, id lengths and the view catalogue.
 */
import { Activity, Film, Gauge, Server, ShieldCheck, Users } from 'lucide-react';
import type { View, ViewTab } from './types';

/** How often the Control tab reloads fleet, sessions, audit and recordings. */
export const CONTROL_POLL_MS = 4000;
/** How often Project operations reloads the durable inventory. */
export const DURABLE_POLL_MS = 5000;

/** Error rate (percent) above which the command-error stat turns red. */
export const ERROR_RATE_BAD = 10;
/** Error rate (percent) above which the command-error stat turns yellow. */
export const ERROR_RATE_WARN = 2;
/** Capacity use (percent) above which a provider bar turns red. */
export const BAR_BAD_PCT = 90;
/** Capacity use (percent) above which a provider bar turns amber. */
export const BAR_WARN_PCT = 70;
/** A limit whose remaining allowance is under this share of its burst is highlighted. */
export const LOW_REMAINING_SHARE = 0.2;
/** Whole percent, for turning a ratio into a percentage. */
export const PERCENT = 100;

/** Bytes per kilobyte, for the traffic and recording sizes. */
export const BYTES_PER_KB = 1024;
/** Byte units, smallest first. */
export const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];
/** Seconds per minute. */
export const SECONDS_PER_MINUTE = 60;
/** Seconds per hour. */
export const SECONDS_PER_HOUR = 3600;
/** Milliseconds per second. */
export const MS_PER_SECOND = 1000;

/** Characters of a CDP session id shown in the sessions table. */
export const SESSION_ID_CHARS = 8;
/** Characters of an actor id shown in the audit table. */
export const ACTOR_ID_CHARS = 10;
/** Characters of an id shown on recordings, durable sessions and events. */
export const SHORT_ID_CHARS = 12;
/** Characters of the session id under the recording player's title. */
export const PLAYER_ID_CHARS = 16;
/** Columns in the CDP sessions table, for the empty row. */
export const SESSION_COLUMNS = 7;
/** Columns in the audit table, for the empty row. */
export const AUDIT_COLUMNS = 6;
/** Longest provider name the form accepts. */
export const PROVIDER_NAME_MAX = 80;

/** Shortest pause between recording frames, so playback never races. */
export const FRAME_GAP_MIN_MS = 80;
/** Longest pause between recording frames, so idle stretches still move. */
export const FRAME_GAP_MAX_MS = 1000;
/** Pause used when two frames carry no usable timing. */
export const FRAME_GAP_DEFAULT_MS = 200;
/** Frames needed before playback has anything to animate. */
export const MIN_FRAMES_TO_PLAY = 2;
/** Intrinsic frame size handed to next/image; CSS scales it down. */
export const FRAME_WIDTH = 1920;
/** Intrinsic frame height handed to next/image. */
export const FRAME_HEIGHT = 1080;

/** Durable events listed under Durable activity, most recent first. */
export const DURABLE_EVENTS_SHOWN = 30;
/** Undelivered webhook deliveries listed for replay. */
export const DELIVERIES_SHOWN = 20;

/** Routing strategies the gateway accepts. */
export const STRATEGIES = ['priority', 'round-robin', 'least-connections', 'latency', 'weighted'];

/** The Control tab's views, in tab order. */
export const VIEWS: ViewTab[] = [
  { key: 'operations', label: 'Project operations', icon: ShieldCheck },
  { key: 'health', label: 'Overview', icon: Activity },
  { key: 'sessions', label: 'CDP sessions', icon: Users },
  { key: 'providers', label: 'Providers', icon: Server },
  { key: 'usage', label: 'Usage', icon: Gauge },
  { key: 'audit', label: 'Audit', icon: ShieldCheck },
  { key: 'recordings', label: 'Recordings', icon: Film },
];

/** The sentence under each view's heading. */
export const VIEW_DESCRIPTIONS: Record<View, string> = {
  operations: 'Durable sessions, access controls, budgets, and event delivery.',
  health: 'A clear view of browser health, capacity, and usage.',
  sessions:
    'Persistent CDP connections from clients such as Playwright and Puppeteer. Individual REST or curl commands do not create a session; find them in the browser’s Activity history.',
  providers:
    'Route new Playwright and Puppeteer connections through your providers. The Start browser default is managed separately in Settings.',
  usage: 'Commands, browser time, and model usage for the current hour.',
  audit: 'A timeline of workspace changes and administrative actions.',
  recordings: 'Review recordings captured from your CDP sessions.',
};

/** A blank Add provider form. */
export const EMPTY_DRAFT = {
  name: '',
  type: 'cdp',
  wsUrl: '',
  apiKey: '',
  maxConcurrent: '5',
  priority: '100',
  weight: '1',
};

/** Usage table rows in order: label and the counter behind it; time and bytes are formatted from their own counters. */
export const USAGE_ROWS: [string, string | null][] = [
  ['Browsers started', 'browsers_started'],
  ['Browsers open now', 'openBrowsers'],
  ['Browser time', null],
  ['Commands', 'commands'],
  ['Command errors', 'command_errors'],
  ['Chat requests', 'chat_requests'],
  ['Model tokens in', 'chat_input_tokens'],
  ['Model tokens out', 'chat_output_tokens'],
  ['Cookie pulls', 'cookie_pulls'],
  ['Frames', 'frames'],
  ['Sandboxes created', 'sandboxes_created'],
  ['Rate limited', 'rate_limited'],
  ['Quota denied', 'quota_denied'],
  ['Bytes out', null],
];

/** What one Control poll fetches, in the order loadControl unpacks it. */
export const CONTROL_PATHS = ['/fleet', '/gateway/sessions', '/audit?limit=200', '/gateway/recordings', '/providers'];

/** Numeric fields on the Add provider form, with their labels. */
export const DRAFT_NUMBERS = [
  ['maxConcurrent', 'Max sessions'],
  ['priority', 'Priority (lower wins)'],
  ['weight', 'Weight'],
] as const;

/** Vendors the Add provider form offers, besides a raw CDP endpoint. */
export const PROVIDER_TYPES = [
  ['cdp', 'cdp, your own CDP endpoint'],
  ['anchor', 'Anchor'],
  ['browserbase', 'Browserbase'],
  ['steel', 'Steel'],
  ['browseruse', 'Browser Use'],
] as const;

/** Durable session states, for the inventory filter. */
export const SESSION_STATES = [
  'queued',
  'provisioning',
  'ready',
  'disconnected',
  'cleanup_pending',
  'unknown_outcome',
  'stopped',
  'failed',
];
/** States in which a durable session is over. */
export const ENDED_STATES = ['stopped', 'failed'];
/** States that need an operator to reconcile them. */
export const RECONCILE_STATES = ['cleanup_pending', 'unknown_outcome'];
/** Roles a service credential or invitation can carry. */
export const CREDENTIAL_ROLES = ['viewer', 'operator', 'administrator'];
/** Project settings edited as numbers, with their labels. */
export const LIMIT_FIELDS = {
  maxConcurrent: 'Concurrent browsers',
  budgetUsd: 'Project budget · USD',
  recordingDays: 'Recording retention · days',
  auditDays: 'Audit retention · days',
} as const;

/** Class for every small button in Project operations. */
export const DURABLE_BUTTON =
  'rounded border border-border px-3 py-1.5 text-xs hover:bg-bg-elevated disabled:opacity-40';
/** Class for every input and select in Project operations. */
export const DURABLE_FIELD = 'w-full rounded border border-border bg-bg p-2 text-sm';
/** Indent for the settings JSON shown in the editors. */
export const JSON_INDENT = 2;
/** Decimal places on the estimated cost. */
export const COST_DECIMALS = 2;
