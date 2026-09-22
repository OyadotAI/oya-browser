/**
 * Types for the durable control plane: sessions and their lifecycle, human
 * takeover, project settings, lifecycle events and service credentials.
 */

/** What a member or credential may do: read, operate browsers, or also administer the project. */
export type ControlRole = 'viewer' | 'operator' | 'administrator';

/** How to recover a session: in place (the default), or replaced by a fresh one. */
export interface RecoverOptions {
  /** Start a fresh session in its place. */
  replace?: boolean;
  /** cdp only: the Chrome the replacement runs on. */
  wsUrl?: string;
}

/** What a human holding the control lease may send. Mirrors the server's allowlist. */
export type HumanInputAction =
  | 'click'
  | 'type'
  | 'press_key'
  | 'scroll'
  | 'click_coordinates'
  | 'double_click'
  | 'drag'
  | 'mouse_move'
  | 'scroll_at'
  | 'type_text'
  | 'keyboard_type'
  | 'navigate'
  | 'back'
  | 'forward'
  | 'reload'
  | 'screenshot'
  | 'analyze'
  | 'read_page';

/** A browser session as the control plane records it, including ones no longer connected. */
export interface ControlSession {
  /** The session's id, which is also the browser's. */
  id: string;
  /** The project it belongs to. */
  project: string;
  /** Which provider runs it. */
  provider: string;
  /** The persona it runs as. */
  persona: string | null;
  /** Where it is in its lifecycle. */
  state:
    | 'queued'
    | 'provisioning'
    | 'ready'
    | 'disconnected'
    | 'stopping'
    | 'cleanup_pending'
    | 'stopped'
    | 'failed'
    | 'unknown_outcome';
  /** Whether the control plane provisioned it (and so must clean it up). */
  managed: boolean;
  /** When it was created, in epoch milliseconds. */
  createdAt: number;
  /** When its state last changed. */
  updatedAt: number;
  /** Estimated spend so far, in US dollars. */
  costUsd: number;
  /** Who is driving it. */
  control: {
    /** The agent, a person, or nobody while paused. */
    mode: 'agent' | 'human' | 'paused';
    /** When a human lease runs out. */
    expiresAt?: number;
  };
  /** Why cleanup has not finished. */
  cleanupError?: string;
}

/** A project's limits, retention and rate cards. */
export interface ProjectSettings {
  /** Days recordings are kept. */
  recordingDays: number;
  /** Days audit events are kept. */
  auditDays: number;
  /** Spend ceiling in US dollars; null for none. */
  budgetUsd: number | null;
  /** Browsers that may run at once; null for no cap. */
  maxConcurrent: number | null;
  /** Price per unit, by what is metered. */
  rates: Record<string, number>;
  /** The default policy for governed browsers. */
  policy: Record<string, unknown>;
}

/** One durable lifecycle event. */
export interface ControlEvent {
  /** Increasing id, used as the read cursor. */
  id: number;
  /** The project it belongs to. */
  project: string;
  /** What happened. */
  type: string;
  /** The session it concerns, if any. */
  sessionId: string | null;
  /** When, in epoch milliseconds. */
  at: number;
  /** Event-specific fields. */
  detail: Record<string, unknown>;
}

/** A service credential. Its token is shown once, at creation. */
export interface ControlCredential {
  /** The credential's id, for revoking it. */
  id: string;
  /** What it is for. */
  label: string;
  /** What it may do. */
  role: ControlRole;
  /** When it stops working; null for never. */
  expiresAt: number | null;
  /** When it was revoked, if it was. */
  revokedAt: number | null;
}

/** The project at a glance: settings, sessions, recent events. */
export interface ControlOverview {
  /** costUsd: estimated lifetime spend, metered from rate cards. */
  project: {
    /** The project's id. */
    id: string;
    /** Its name. */
    name: string;
    /** Its limits and retention. */
    settings: ProjectSettings;
    /** Estimated lifetime spend, in US dollars. */
    costUsd?: number;
  };
  /** Every session the control plane knows about. */
  sessions: ControlSession[];
  /** Recent lifecycle events. */
  events: ControlEvent[];
  /** True while the server is draining for a restart. */
  draining: boolean;
  /** Service credentials, for administrators. */
  credentials?: ControlCredential[];
}
