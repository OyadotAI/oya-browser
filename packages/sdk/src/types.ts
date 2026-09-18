/** Everything the API returns or accepts, in one place. */

/** Which model drives `ask()` and the chat API. */
export type LlmProvider =
  | 'openai'
  | 'anthropic'
  /** Gemini via AI Studio. */
  | 'gemini'
  /** Gemini Enterprise, ex-Vertex AI. Express mode by default; set `openai_base_url` to a
   *  project-scoped `.../endpoints/openapi` endpoint to use an enterprise project. */
  | 'vertex';

/** Empty disables solving. */
export type CaptchaSolver = 'capsolver' | '2captcha' | '';

/**
 * What a key can configure. Mirrors the server's field allowlist exactly: a field not
 * listed here is ignored rather than stored, so the type is the whole surface.
 * `null` clears a field and falls back to the deployment default.
 */
export interface ConfigUpdate {
  llm_provider?: LlmProvider | null;
  /** The credential for whichever `llm_provider` is set — the field name is shared. */
  openai_api_key?: string | null;
  /** Only honoured alongside this key's own `openai_api_key`. */
  openai_base_url?: string | null;
  /** Overrides the provider's default model. */
  chat_model?: string | null;
  browser_provider?: Provider | null;
  anchor_api_key?: string | null;
  browserbase_api_key?: string | null;
  browserbase_project_id?: string | null;
  steel_api_key?: string | null;
  browseruse_api_key?: string | null;
  cdp_ws_url?: string | null;
  captcha_solver?: CaptchaSolver | null;
  captcha_api_key?: string | null;
  onboarded?: string | null;
}

/** What `config.get()` returns. Secrets read back masked, never in full. */
export interface Config extends Omit<ConfigUpdate, 'llm_provider' | 'browser_provider' | 'captcha_solver'> {
  llm_provider?: LlmProvider | '';
  browser_provider?: Provider | '';
  captcha_solver?: CaptchaSolver;
  /** What this key would actually use right now, deployment defaults included. */
  effective: { baseUrl: string; model: string; hasLlmKey: boolean };
  /** True when the LLM key in play belongs to the deployment, not this key. */
  inherited: boolean;
  has_openai_key: boolean;
  providers: Array<{ id: Provider; label: string; needs: string[]; configured: boolean }>;
}

/** Where a browser comes from. Configuration, not something a caller must know. */
export type Provider =
  | 'oya-cloud'
  | 'oya-selfhosted'
  | 'browseruse'
  | 'browserbase'
  | 'steel'
  | 'anchor'
  | 'cdp';

export interface StartOptions {
  /** Reuse this value when retrying the same logical creation. */
  idempotencyKey?: string;
  /** Wait for capacity for up to five minutes; zero rejects immediately. */
  queueMs?: number;
  budgetUsd?: number;
  governed?: boolean;
  policy?: { allowedHosts?: string[]; humanHosts?: string[]; region?: string; redactRecording?: boolean };
  priority?: 'low' | 'normal' | 'high';
  /** Saved login profile. Defaults to the desktop's default profile. */
  profile?: string;
  /**
   * Which identity to run as. A persona is one device: fingerprint, cookie jar
   * and proxy bound together and stable for its life.
   *   'default' — this key's own persona (the default)
   *   'auto'    — the least recently used persona under its concurrency cap
   *   <id>      — a specific persona
   */
  persona?: 'default' | 'auto' | (string & {});
  /** Solve CAPTCHAs as they appear rather than waiting to be asked. */
  captcha?: 'auto' | 'off';
  /** Override the key's configured provider for this browser only. */
  provider?: Provider;
  /** Required only for the 'cdp' provider. */
  wsUrl?: string;
  name?: string;
  /** How long to wait for a cloud browser to dial in. Default 120s. */
  readyTimeoutMs?: number;
}

export interface StartResult {
  id: string;
  provider: string;
  persona: string;
  status: 'ready' | 'starting';
  /** Point Playwright, Puppeteer or browser-use at this. */
  cdpUrl?: string;
  note?: string;
}

export interface Playbook {
  name: string;
  /** Inputs `play()` accepts; any left out reuse the recorded value. */
  variables: string[];
  /** What each variable was recorded with. A secret has none — it never left the page. */
  defaults: Record<string, string>;
  steps: number;
  /** The same flow as a Playwright module: `export default async function run(page, vars)`. */
  code: string;
}

export interface PlayResult {
  /** Steps replayed before finishing or handing over to the agent. */
  steps: number;
  total: number;
  /** A step no longer fit the page and the agent finished the task. */
  fellBack: boolean;
  /** The agent's fix was saved as `draft`; promote it with `oya.playbooks.promote(name)`. */
  healed?: boolean;
  draft?: string;
  /** The agent's summary, when it fell back. */
  text?: string;
}

export interface PlaybookSummary extends Playbook {
  createdAt: string | null;
  promotedAt: string | null;
  /** A healed replay's fix, waiting for `promote()` or `remove('<name>:draft')`. */
  draft: (Playbook & { healedAt: string; healedFrom: number }) | null;
}

/**
 * A file attached to a task value. Build it with `file()`, never by hand. Only `data`
 * takes one: a file is not typed through a placeholder, so `secrets` has nothing to hide
 * and rejects it.
 */
export interface FileValue {
  /** The filename the site sees. */
  file: string;
  /** MIME type, guessed from the extension unless you pass one. */
  type: string;
  /** The bytes, base64. 10MB ceiling. */
  b64: string;
}

/**
 * Task values, referred to as `{{name}}` in prompts. As `data` the agent can read them
 * (to split a name or pick the right option); as `secrets` it never sees them. Either
 * way they are typed through placeholders, so playbooks store no values.
 *
 * A {@link FileValue} from `file()` is the exception: the agent attaches it with its
 * upload tool rather than typing it.
 */
export type RunData = Record<string, string | number | FileValue>;

export interface AttentionRequest {
  id: string;
  /** captcha / login / mfa: finish it in the live view. agent: the agent's question. heal_failed: replay and the agent both gave up. */
  reason: 'captcha' | 'login' | 'mfa' | 'agent' | 'heal_failed';
  message: string;
  liveViewUrl?: string;
  at: number;
}

export type RunResult = Partial<PlayResult> & { text?: string };

export interface RunInfo {
  id: string;
  browserId: string;
  status: 'running' | 'needs_attention' | 'succeeded' | 'failed';
  createdAt: number;
  endedAt?: number;
  attention: AttentionRequest | null;
  result?: RunResult;
  error?: string;
  /** HTTP-style status of a failure: 429 when a quota stopped the run. */
  errorStatus?: number;
}

export interface SubmitOptions {
  /** Task values the agent can read; for a playbook, its variables (secret ones included). */
  data?: RunData;
  /** Prompts only: values the agent never sees, like passwords. A playbook already knows which of its variables are secret. */
  secrets?: RunData;
  /** Playbooks only: let the agent finish a broken replay and save its fix as a draft. Default true. */
  autoHeal?: boolean;
  onSuccess?: (result: RunResult) => unknown;
  onFailure?: (error: OyaError) => unknown;
  /** Call `respond()` once it is handled: `'done'` after finishing by hand, or your answer to the agent. */
  onHumanAttention?: (request: AttentionRequest & { respond(response?: string): Promise<void> }) => unknown;
  /** Fires before onSuccess when a replay was healed; `result.draft` names the draft. */
  onHealed?: (result: RunResult) => unknown;
  /** How often to check on the run. Default 2000. */
  pollMs?: number;
}

export interface Element {
  id: number;
  type: string;
  /** Visible label, capped at 80 characters by the analyzer. */
  text?: string;
  href?: string;
  value?: string;
  checked?: boolean;
  disabled?: boolean;
  visible: boolean;
  tag?: string;
  /** The element's DOM `id`. Usually the most stable handle a site offers. */
  domId?: string;
  ariaLabel?: string;
  /** `data-testid`, when the site ships one. */
  testId?: string;
  name?: string;
  placeholder?: string;
  /** Action of the enclosing form. */
  formName?: string;
}

export interface Analysis {
  /** The page as markdown. */
  markdown: string;
  elements: Element[];
  viewport?: { width: number; height: number };
  scroll?: { x: number; y: number };
  truncated?: boolean;
}

export interface CaptchaResult {
  present: boolean;
  solved: boolean;
  /** 'provider' when the vendor solved it, 'solver' when we did, 'none' otherwise. */
  method: 'provider' | 'solver' | 'none';
  type?: string;
  /** Invisible reCAPTCHA v3 scores the visit passively; there is nothing on screen to clear. */
  invisible?: boolean;
  sitekey?: string | null;
  error?: string;
}

export interface MfaResult {
  present: boolean;
  completed: boolean;
  method?: 'totp' | 'email' | 'sms' | 'handoff' | 'none';
  filled?: boolean;
  submitted?: boolean;
  /** Open this to finish by hand when nothing automated can. */
  liveViewUrl?: string | null;
  error?: string;
}

export interface Fingerprint {
  platform: string; timezone: string; locale: string; screen: string; webgl: string;
  hardwareConcurrency: number; deviceMemory: number; canvasSeed: number;
}

/** Device choices made at creation. Fixed for the persona's life. */
export interface PersonaPrefs { platform?: 'Win32' | 'MacIntel' | 'Linux x86_64'; timezone?: string; locale?: string }

/** A proxy exit. Credentials go in on create and never come back out. */
export interface ProxyInfo {
  id: string;
  label: string;
  kind: 'residential' | 'datacenter';
  /** Two-letter country, optionally a region: "US", "US-CA". */
  geo: string | null;
  /** Provided by the host rather than this key. Cannot be removed. */
  shared: boolean;
  healthy: boolean;
  /** Healthy and not cooling down after a failure. */
  available: boolean;
  /** Where traffic actually leaves, as of the last check. */
  exitIp: string | null;
  lastCheckedAt: string | null;
  /** Personas on it now, out of `maxPersonas`. */
  assigned: number;
  maxPersonas: number;
  cooldownMsRemaining: number;
}

export interface ProxyCreate {
  /** http(s)://user:pass@host:port from your vendor. Chromium cannot use SOCKS5 with a password. */
  url: string;
  label?: string;
  geo?: string;
  kind?: 'residential' | 'datacenter';
  /** Personas that may share it. Keep 1 for a sticky-session URL so each keeps its own IP. */
  maxPersonas?: number;
}

export interface PersonaInfo {
  id: string;
  name: string;
  isDefault: boolean;
  activeBrowsers: number;
  maxConcurrent: number | null;
  proxy: { geo: string | null } | null;
  /** The proxy it is actually on, once assigned or pinned. */
  exit: { id: string; label: string; geo: string | null; healthy: boolean } | null;
  prefs: PersonaPrefs | null;
  fingerprint: Fingerprint;
  mfa: { configured: boolean; type?: string; domain?: string };
  /** Per-site factors and stored logins. Usernames and types only, never secrets. */
  sites: {
    mfa: { domain: string; type: string }[];
    credentials: { domain: string; username: string }[];
  };
  login: { cookies: number; sites: string[]; updatedAt: string | null };
  createdAt: string;
  lastUsedAt: string | null;
}

export type Health = 'ok' | 'stale' | 'errors' | 'dead';

export interface Activity { ts: string; action: string; summary: string; ok: boolean; ms: number; error?: string }

export interface StopResult { id: string; ok: boolean; provider?: string | null; sandboxRemoved?: boolean | null; error?: string }

/**
 * A second factor. `domain` files it against one site, because a persona driving
 * several portals meets several kinds of factor; without it the record is the
 * persona-wide default.
 *
 * `gmail` and `graph` read the code straight out of a mailbox. `email` and `sms`
 * poll an endpoint you host — set `x-oya-received-at` on its response (epoch ms)
 * and a code from a previous run will never be reused.
 *
 * The code is pulled out of the message by your own configured LLM, because
 * portals rewrite these templates constantly and the code is not always digits.
 * `pattern` is only the fallback for when no LLM key is set or the call fails.
 */
export type MfaConfig = { domain?: string } & (
  | { type: 'totp'; secret: string }
  | { type: 'email' | 'sms'; url: string; headers?: Record<string, string>; pattern?: string; timeoutMs?: number }
  | { type: 'gmail' | 'graph'; refreshToken: string; clientId: string; clientSecret?: string; tenant?: string; query?: string; pattern?: string; timeoutMs?: number }
);

/** A site login. The password is write-only: no API ever reads it back. */
export interface SiteCredentials { domain: string; username: string; password: string }

export interface BrowserInfo {
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
}

export interface BrowserDetail extends BrowserInfo { activity: Activity[] }

export interface OyaOptions {
  /** Defaults to OYA_API_KEY. */
  apiKey?: string;
  /** Defaults to OYA_BASE_URL, then https://browser.getoya.ai. */
  baseUrl?: string;
  /** Per-request timeout. Navigation gets its own, longer budget. */
  timeoutMs?: number;
  fetch?: typeof globalThis.fetch;
}

export class OyaError extends Error {
  readonly status: number;
  readonly body: unknown;
  constructor(message: string, status: number, body: unknown) {
    super(message);
    this.name = 'OyaError';
    this.status = status;
    this.body = body;
  }
}

export type ControlRole = 'viewer' | 'operator' | 'administrator';
/** What a human holding the control lease may send. Mirrors the server's allowlist. */
export type HumanInputAction =
  | 'click' | 'type' | 'press_key' | 'scroll' | 'click_coordinates' | 'double_click' | 'drag'
  | 'mouse_move' | 'scroll_at' | 'type_text' | 'keyboard_type' | 'navigate' | 'back' | 'forward'
  | 'reload' | 'screenshot' | 'analyze' | 'read_page';
export interface ControlSession {
  id: string; project: string; provider: string; persona: string | null;
  state: 'queued' | 'provisioning' | 'ready' | 'disconnected' | 'stopping' | 'cleanup_pending' | 'stopped' | 'failed' | 'unknown_outcome';
  managed: boolean; createdAt: number; updatedAt: number; costUsd: number;
  control: { mode: 'agent' | 'human' | 'paused'; expiresAt?: number };
  cleanupError?: string;
}
export interface ProjectSettings {
  recordingDays: number; auditDays: number; budgetUsd: number | null;
  maxConcurrent: number | null; rates: Record<string, number>; policy: Record<string, unknown>;
}
export interface ControlEvent { id: number; project: string; type: string; sessionId: string | null; at: number; detail: Record<string, unknown> }
export interface ControlCredential { id: string; label: string; role: ControlRole; expiresAt: number | null; revokedAt: number | null }
export interface ControlOverview {
  /** costUsd: estimated lifetime spend, metered from rate cards. */
  project: { id: string; name: string; settings: ProjectSettings; costUsd?: number };
  sessions: ControlSession[]; events: ControlEvent[]; draining: boolean;
  credentials?: ControlCredential[];
}
