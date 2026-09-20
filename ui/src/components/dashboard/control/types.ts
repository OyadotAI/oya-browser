/**
 * Shapes the Control tab reads from the server: the fleet summary, gateway
 * sessions, providers, audit events, recordings and the durable project
 * overview. Everything is scoped to the connected API key.
 */

import type { Activity } from 'lucide-react';

/** A tab in the Control strip. */
export interface ViewTab {
  /** The view it opens. */
  key: View;
  /** Its name, also the heading. */
  label: string;
  /** Its icon. */
  icon: typeof Activity;
}

/** One of the Control tab's views. */
export type View = 'operations' | 'health' | 'sessions' | 'providers' | 'usage' | 'audit' | 'recordings';

/** A per-minute limit and what is left of it. */
export interface Limit {
  /** Requests allowed per minute. */
  limit: number;
  /** Extra requests allowed in a burst. */
  burst?: number;
  /** Requests left right now. */
  remaining: number;
  /** The limit is switched off for this key. */
  disabled?: boolean;
}

/** GET /fleet: the key's browsers, sessions, routing, usage, limits and quotas. */
export interface ControlFleet {
  /** When the server took the snapshot. */
  at: string;
  /** How long the host has been up. */
  uptimeSeconds: number;
  /** Connected browsers. */
  browsers: {
    /** How many are connected. */
    total: number;
    /** Count per client type. */
    byClient: Record<string, number>;
    /** Count per provider. */
    byProvider: Record<string, number>;
  };
  /** CDP gateway sessions. */
  sessions: {
    /** How many exist. */
    total: number;
    /** How many have a client attached. */
    attached: number;
    /** How many are recording. */
    recording: number;
  };
  /** The gateway's provider routing. */
  routing: Routing;
  /** Counters for the current hour, by metric name. */
  usage: Usage;
  /** Rate limits by name. */
  limits: Record<string, Limit>;
  /** Hourly quotas by name. */
  quotas: Record<string, number>;
}

/** Usage counters for the current hour. */
export type Usage = Record<string, number> & {
  /** Start of the hour, ISO. */
  hour: string;
  /** Browsers open right now. */
  openBrowsers: number;
};

/** A CDP gateway session. */
export interface Session {
  /** Session id. */
  id: string;
  /** Provider it runs on. */
  provider: string;
  /** Profile it started with, if any. */
  profile: string | null;
  /** A client is attached; otherwise it is held for resume. */
  connected: boolean;
  /** Age in seconds. */
  seconds: number;
  /** Bytes sent to the browser. */
  bytesUp: number;
  /** Bytes received from the browser. */
  bytesDown: number;
  /** Frames are being recorded. */
  recording: boolean;
}

/** A routing provider and its live state. */
export interface Provider {
  /** Provider name, unique per owner. */
  name: string;
  /** Vendor type, or cdp. */
  type: string;
  /** Declared by the host for every key. */
  shared?: boolean;
  /** Key that owns it, if not shared. */
  owner?: string | null;
  /** Sessions running on it now. */
  active: number;
  /** Sessions it may run at once. */
  maxConcurrent: number;
  /** Lower wins under the priority strategy. */
  priority: number;
  /** Share under the weighted strategy. */
  weight: number;
  /** Its last connection worked. */
  healthy: boolean;
  /** It can take a new session. */
  available: boolean;
  /** Last connect latency. */
  latencyMs: number | null;
  /** Time left before a failed provider is retried. */
  cooldownMsRemaining: number;
  /** Sessions it has ever run. */
  totalSessions: number;
  /** Connections that failed. */
  totalFailures: number;
}

/** A vendor from GET /providers, and whether the key has its credential. */
export interface ProviderChoice {
  /** Vendor id. */
  name: string;
  /** A credential is saved for it. */
  configured: boolean;
}

/** The gateway's routing state. */
export interface Routing {
  /** Active strategy name. */
  strategy: string;
  /** Connections waiting for a slot. */
  queueDepth: number;
  /** Slots across all providers. */
  capacity: number;
  /** Slots in use. */
  active: number;
  /** Healthy providers. */
  healthy: number;
  /** Every provider the key can route to. */
  providers: Provider[];
}

/** One entry in the key's audit trail. */
export interface AuditEvent {
  /** When it happened. */
  ts: string;
  /** What was done. */
  action: string;
  /** Who did it. */
  actor: string | null;
  /** Kind of thing it was done to. */
  target_type: string | null;
  /** Id of the thing it was done to. */
  target_id: string | null;
  /** ok, denied or an error. */
  outcome: string;
  /** Address it came from. */
  ip: string | null;
  /** Extra detail. */
  meta: Record<string, unknown> | null;
}

/** A recorded CDP session. */
export interface Recording {
  /** Session it was recorded from. */
  sessionId: string;
  /** Provider the session ran on. */
  provider?: string;
  /** Profile the session used. */
  profile?: string | null;
  /** When recording started. */
  startedAt?: string;
  /** Length in milliseconds. */
  durationMs?: number;
  /** Frames captured. */
  frameCount?: number;
  /** Size on disk. */
  bytes?: number;
  /** Still recording. */
  live?: boolean;
  /** Stopped at the frame cap. */
  truncated?: boolean;
}

/** A frame in a recording: its index and time offset in milliseconds. */
export interface Frame {
  /** Frame index on the server. */
  i: number;
  /** Milliseconds from the start. */
  t: number;
}

/** The Add provider form, as the user types it. */
export interface ProviderDraft {
  /** Provider name. */
  name: string;
  /** cdp or a vendor id. */
  type: string;
  /** CDP WebSocket URL, for cdp. */
  wsUrl: string;
  /** Vendor API key, for vendors. */
  apiKey: string;
  /** Max sessions, as typed. */
  maxConcurrent: string;
  /** Priority, as typed. */
  priority: string;
  /** Weight, as typed. */
  weight: string;
}

/** A durable session in the project inventory. */
export interface DurableSession {
  /** Session id. */
  id: string;
  /** Provider it runs on. */
  provider: string;
  /** Lifecycle state. */
  state: string;
  /** Provisioned by the control plane, with its guarantees. */
  managed: boolean;
  /** Estimated cost so far. */
  costUsd: number;
  /** Who is driving it. */
  control: {
    /** agent, human, or paused. */
    mode: string;
  };
  /** Why cleanup failed, if it did. */
  cleanupError?: string;
}

/** Project settings an administrator can change. */
export interface ProjectSettings {
  /** Concurrent browsers, or no override. */
  maxConcurrent: number | null;
  /** Hard budget, or none. */
  budgetUsd: number | null;
  /** Days recordings are kept. */
  recordingDays: number;
  /** Days audit events are kept. */
  auditDays: number;
  /** USD per browser hour, by provider. */
  rates: Record<string, number>;
  /** Managed-browser policy. */
  policy: Record<string, unknown>;
}

/** GET /control: the project's durable state. */
export interface Overview {
  /** The project. */
  project: {
    /** Project id. */
    id: string;
    /** Project name. */
    name: string;
    /** Current settings. */
    settings: ProjectSettings;
    /** Estimated cost so far. */
    costUsd?: number;
  };
  /** Every durable session. */
  sessions: DurableSession[];
  /** Admission is paused while the host drains. */
  draining: boolean;
  /** Durable events, oldest first. */
  events: {
    /** Event id. */
    id: number;
    /** Event type. */
    type: string;
    /** Session it concerns, if any. */
    sessionId: string | null;
    /** When, in epoch ms. */
    at: number;
  }[];
  /** Service credentials; present only for administrators. */
  credentials?: {
    /** Credential id. */
    id: string;
    /** Role it grants. */
    role: string;
    /** Human label. */
    label: string;
    /** When it was revoked, if it was. */
    revokedAt: number | null;
  }[];
  /** Webhooks; present only for administrators. */
  webhooks?: {
    /** Webhook id. */
    id: string;
    /** Where events are sent. */
    url: string;
    /** Still delivering. */
    enabled: boolean;
  }[];
  /** Webhook deliveries. */
  deliveries?: {
    /** Delivery id. */
    id: string;
    /** Delivery state. */
    state: string;
    /** Attempts so far. */
    attempts: number;
  }[];
}

/** A project member. */
export interface Member {
  /** User id. */
  userId: string;
  /** Their role. */
  role: string;
}
