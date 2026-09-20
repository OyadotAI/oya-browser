/**
 * The profile screens' shapes and pure rules: the drafts the forms hold, how a
 * concurrency cap reads, and how a row's numbers are worked out.
 */
import type { BrowserRow, Persona } from '../types';
import type { MfaDraft } from './mfa';

/** A persona's device, as the server reports and previews it. */
export type Fingerprint = Persona['fingerprint'];

/** What GET /personas/options offers: platforms, and the timezones and locales each one really reports. */
export interface Options {
  /** Platforms a device can be. */
  platforms: string[];
  /** Timezones by platform. */
  timezones: Record<string, string[]>;
  /** Locales by platform. */
  locales: Record<string, string[]>;
}

/** The new-profile form, as typed. `auto` lets the server choose. */
export interface PersonaDraft {
  /** Name; blank lets the server name it. */
  name: string;
  /** Platform, or `auto`. */
  platform: string;
  /** Timezone, or `auto`; only offered once a platform is chosen. */
  timezone: string;
  /** Locale, or `auto`; only offered once a platform is chosen. */
  locale: string;
  /** Proxy geo hint; blank means any. */
  geo: string;
  /** Concurrent browsers allowed; blank means uncapped. */
  cap: string;
  /** Optional second factor stored right after creation. */
  mfa: MfaDraft;
}

/** The drawer's editable fields for one persona. */
export interface DrawerFields {
  /** Name. */
  name: string;
  /** Concurrent browsers allowed; blank means uncapped. */
  cap: string;
  /** Proxy geo hint; blank means any. */
  geo: string;
  /** Proxy the persona is pinned to; blank means auto. */
  pin: string;
}

/** A proxy offered in the drawer's exit picker. */
export interface ProxyChoice {
  /** Proxy id. */
  id: string;
  /** Name shown in the picker. */
  label: string;
  /** Country, if known. */
  geo: string | null;
  /** False once the proxy is failing or cooling down. */
  available?: boolean;
  /** Profiles on it now. */
  assigned: number;
  /** Profiles it may serve. */
  maxPersonas: number;
}

/** A proxy as GET /proxies lists it. */
export interface ProxyRow {
  /** Proxy id. */
  id: string;
  /** Name shown in the table. */
  label: string;
  /** Where its IPs come from. */
  kind: 'residential' | 'datacenter';
  /** Country, if known. */
  geo: string | null;
  /** Shared by the server, so this key may not remove it. */
  shared: boolean;
  /** Whether the last check passed. */
  healthy: boolean;
  /** Healthy and not cooling down. */
  available: boolean;
  /** IP it exited from at the last check. */
  exitIp: string | null;
  /** When it was last checked. */
  lastCheckedAt: string | null;
  /** Profiles on it now. */
  assigned: number;
  /** Profiles it may serve. */
  maxPersonas: number;
}

/** The add-a-proxy form, as typed. */
export interface ProxyDraft {
  /** Name; blank lets the server name it. */
  label: string;
  /** Proxy URL with credentials. */
  url: string;
  /** Country; blank means any. */
  geo: string;
  /** Where its IPs come from. */
  kind: 'residential' | 'datacenter';
  /** Profiles it may serve. */
  max: string;
}

/** One proxy's result from POST /proxies/check. */
export interface CheckResult {
  /** Proxy id. */
  id: string;
  /** Whether it worked. */
  ok: boolean;
  /** Why it failed. */
  error?: string;
}

/** What POST /personas/preview answers. */
export interface PreviewAnswer {
  /** The device these preferences would give. */
  fingerprint: Fingerprint;
}

/** What GET /proxies answers; older servers answered a bare list. */
export type ProxyList = ProxyChoice[] | { /** The proxies. */ proxies?: ProxyChoice[] };

/** What GET /proxies answers to the proxies dialog. */
export interface ProxyTable {
  /** Every proxy this key can use. */
  proxies: ProxyRow[];
}

/** What POST /proxies/check answers. */
export interface CheckAnswer {
  /** One result per proxy. */
  results: CheckResult[];
}

/** A portal sign-in as typed, and as PUT /personas/:id/credentials takes it. */
export interface CredentialDraft {
  /** Site it is for. */
  domain: string;
  /** Username. */
  username: string;
  /** Password; sealed once stored. */
  password: string;
}

/** A cap as the form holds it: blank for uncapped. */
export const capText = (max: number | null) => (max === null ? '' : String(max));

/** A typed cap as the server takes it: null for uncapped. */
export const capValue = (cap: string) => (cap === '' ? null : Number(cap));

/** A cap as shown next to a count. */
export const capLabel = (max: number | null) => (max === null ? '∞' : max);

/** The drawer's fields filled from what is stored. */
export const fieldsOf = (p: Persona): DrawerFields => ({
  name: p.name,
  cap: capText(p.maxConcurrent),
  geo: p.proxy?.geo || '',
  pin: p.exit?.id || '',
});

/** Whether the drawer's saveable fields differ from what is stored (the pin applies on its own). */
export const isDirty = (f: DrawerFields, p: Persona) =>
  f.name !== p.name || f.cap !== capText(p.maxConcurrent) || f.geo !== (p.proxy?.geo || '');

/** The browsers running as a persona. */
export const runningAs = (browsers: BrowserRow[], personaId: string) => browsers.filter((b) => b.persona === personaId);

/** A table row's numbers: its cap, whether it is at it, and how many run now. */
export function rowStats(p: Persona, browsers: BrowserRow[]) {
  const cap = p.maxConcurrent === null ? Infinity : p.maxConcurrent;
  const running = runningAs(browsers, p.id).length || p.activeBrowsers;
  return { cap, at: p.activeBrowsers >= cap, running };
}

/** A proxy as the drawer's picker lists it. */
export const choiceLabel = (x: ProxyChoice) =>
  `${x.label}${x.geo ? ` · ${x.geo}` : ''} · ${x.assigned}/${x.maxPersonas}${x.available === false ? ' · unhealthy' : ''}`;

/** The toast after checking every proxy. */
export function checkSummary(results: CheckResult[]): [string, 'error' | 'success'] {
  const bad = results.filter((r) => !r.ok).length;
  if (bad) return [`${bad} of ${results.length} proxies failed`, 'error'];
  return [`All ${results.length} proxies work`, 'success'];
}

/** The account's creation date, written out, or null when unknown. */
export const memberSince = (createdAt: string | undefined) =>
  createdAt
    ? new Date(createdAt).toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' })
    : null;
