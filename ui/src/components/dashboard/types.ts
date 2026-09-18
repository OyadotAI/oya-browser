/** Rows as GET /browsers and GET /browsers/:id return them. */
export type Health = 'ok' | 'stale' | 'errors' | 'dead';

export interface BrowserRow {
  id: string;
  name: string;
  clientType: 'oya' | 'cdp';
  provider: string | null;
  persona: string | null;
  personaName: string | null;
  health: Health;
  connectedAt: string;
  lastSeen: string;
  currentUrl: string;
  commands: number;
  errors: number;
  pending: number;
  lastCommandAt: string | null;
  lastError: string | null;
  streaming: boolean;
}

export interface Activity {
  ts: string;
  action: string;
  summary: string;
  ok: boolean;
  ms: number;
  error?: string;
}

export interface BrowserDetail extends BrowserRow {
  activity: Activity[];
}

export interface Fleet {
  at: string;
  uptimeSeconds: number;
  browsers: {
    total: number;
    byClient: Record<string, number>;
    byProvider: Record<string, number>;
    byHealth: Record<Health, number>;
    byPersona: Record<string, number>;
    commands: number;
    errors: number;
    pending: number;
  };
  usage?: Record<string, number> & { hour?: string };
  quotas?: Record<string, { current?: number; quota?: number; remaining?: number }>;
  limits?: Record<string, unknown>;
}

export interface Persona {
  id: string;
  name: string;
  isDefault: boolean;
  createdAt: string;
  lastUsedAt: string | null;
  activeBrowsers: number;
  maxConcurrent: number | null;
  proxy: { host?: string; port?: number; geo: string | null } | null;
  exit: { id: string; label: string; geo: string | null; healthy: boolean } | null;
  prefs: { platform?: string; timezone?: string; locale?: string } | null;
  fingerprint: {
    platform: string; timezone: string; locale: string; screen: string; webgl: string;
    hardwareConcurrency: number; deviceMemory: number; canvasSeed: number;
  };
  mfa: { configured: boolean; type?: string; domain?: string };
  /** Per-site factors and stored logins. Usernames and types only, never secrets. */
  sites?: {
    mfa: { domain: string; type: string }[];
    credentials: { domain: string; username: string }[];
  };
  login?: { cookies: number; sites: string[]; updatedAt: string | null };
}

export const PROVIDER_LABEL: Record<string, string> = {
  'oya-cloud': 'Oya Cloud',
  'oya-selfhosted': 'Oya self-hosted',
  'oya-desktop': 'Desktop',
  browseruse: 'Browser Use',
  browserbase: 'Browserbase',
  steel: 'Steel',
  anchor: 'Anchor',
  cdp: 'CDP',
};

export const providerLabel = (p: string | null | undefined) => (p ? PROVIDER_LABEL[p] || p : '—');

export const PLATFORM_LABEL: Record<string, string> = {
  Win32: 'Windows', MacIntel: 'macOS', 'Linux x86_64': 'Linux',
};
export const platformLabel = (p: string | undefined) => (p ? PLATFORM_LABEL[p] || p : 'auto');

export const HEALTH_LABEL: Record<Health, string> = { ok: 'healthy', stale: 'stale', errors: 'errors', dead: 'unresponsive' };
