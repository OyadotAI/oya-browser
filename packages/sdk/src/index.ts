/**
 * @oya-ai/browser — thousands of browsers, one API.
 *
 *   import { Oya } from '@oya-ai/browser';
 *
 *   const oya = new Oya();                                    // OYA_API_KEY
 *   const browser = await oya.browser.start({ persona: 'auto', captcha: 'auto' });
 *   await browser.goto('https://example.com');
 *
 * Which provider actually runs the browser — Oya Cloud, your own machines,
 * Browser Use, Browserbase, Steel, Anchor, or a CDP URL you hand us — is
 * configuration on your API key, not something this code has to know.
 */

import { Http } from './client.js';
import { Browser, Run } from './browser.js';
import {
  OyaError,
  type ControlOverview, type ControlSession, type ControlRole, type ControlCredential, type HumanInputAction, type ProjectSettings, type ControlEvent,
  type BrowserInfo, type Fingerprint, type MfaConfig, type OyaOptions,
  type PersonaInfo, type PersonaPrefs, type Playbook, type PlaybookSummary, type ProxyInfo, type ProxyCreate, type StartOptions, type StartResult, type StopResult,
} from './types.js';

export { Browser, Run, OyaError };
export * from './types.js';

const DEFAULT_BASE_URL = 'https://browser.getoya.ai';
const READY_POLL_MS = 2_000;

const env = (name: string): string | undefined =>
  (globalThis as { process?: { env?: Record<string, string | undefined> } }).process?.env?.[name];

export class Oya {
  private readonly http: Http;

  constructor(options: OyaOptions = {}) {
    const apiKey = options.apiKey || env('OYA_API_KEY');
    if (!apiKey) {
      throw new Error('No API key. Pass { apiKey } or set OYA_API_KEY — run `oya login` to get one.');
    }
    const baseUrl = (options.baseUrl || env('OYA_BASE_URL') || DEFAULT_BASE_URL).replace(/\/+$/, '');
    const fetchImpl = options.fetch || globalThis.fetch;
    if (!fetchImpl) throw new Error('No fetch available — pass { fetch } or use Node 18+.');
    this.http = new Http(baseUrl, apiKey, options.timeoutMs ?? 60_000, fetchImpl.bind(globalThis));
  }

  readonly browser = {
    /** Start a browser and wait until it can take commands. */
    start: async (options: StartOptions = {}): Promise<Browser> => {
      const started = await this.http.request<StartResult>('POST', '/api/browsers/start', {
        profile: options.profile || options.persona,
        provider: options.provider,
        wsUrl: options.wsUrl,
        name: options.name,
        queueMs: options.queueMs, priority: options.priority,
        budgetUsd: options.budgetUsd, governed: options.governed, policy: options.policy,
      }, 120_000, { 'Idempotency-Key': options.idempotencyKey || globalThis.crypto.randomUUID() });

      // Cloud browsers dial in themselves, so 'starting' means "not yet".
      if (started.status === 'starting') {
        await this.waitUntilConnected(started.id, options.readyTimeoutMs ?? (120_000 + (options.queueMs || 0)));
        const connected = await this.http.request<BrowserInfo & { cdpUrl?: string }>('GET', `/api/browsers/${encodeURIComponent(started.id)}`);
        started.cdpUrl = connected.cdpUrl;
      }
      return new Browser(this.http, started, options.captcha === 'auto');
    },

    /** Reattach to a browser that is already running. */
    get: async (id: string): Promise<Browser> => {
      const found = await this.http.request<BrowserInfo & { cdpUrl?: string }>('GET', `/api/browsers/${encodeURIComponent(id)}`);
      return new Browser(this.http, {
        id: found.id, provider: found.provider || 'cdp', persona: found.persona || 'default', status: 'ready', cdpUrl: found.cdpUrl,
      }, false);
    },

    list: (): Promise<BrowserInfo[]> => this.http.request<BrowserInfo[]>('GET', '/api/browsers'),

    /** Stop some (`ids`) or every browser on this key. Each reports separately. */
    stop: (ids: string[] | 'all'): Promise<{ stopped: number; results: StopResult[] }> =>
      this.http.request('POST', '/api/browsers/stop', ids === 'all' ? { all: true } : { ids }, 120_000),

    stopAll: async (): Promise<number> => (await this.browser.stop('all')).stopped,
  };

  /** Durable operational controls, including disconnected and cleanup-pending sessions. */
  readonly control = {
    overview: (): Promise<ControlOverview> => this.http.request('GET', '/api/control'),
    sessions: (): Promise<ControlSession[]> => this.http.request('GET', '/api/control/sessions'),
    session: (id: string): Promise<ControlSession> => this.http.request('GET', `/api/control/sessions/${encodeURIComponent(id)}`),
    settings: (changes: Partial<ProjectSettings>): Promise<ControlOverview['project']> => this.http.request('PATCH', '/api/control/project', changes),
    cancel: (id: string): Promise<ControlSession> => this.http.request('POST', `/api/control/sessions/${encodeURIComponent(id)}/cancel`, {}),
    stop: (id: string, force = false): Promise<StopResult> => this.http.request('POST', `/api/control/sessions/${encodeURIComponent(id)}/stop`, { force }),
    takeover: (id: string, action: 'acquire' | 'release' | 'resume'): Promise<ControlSession['control']> => this.http.request('POST', `/api/control/sessions/${encodeURIComponent(id)}/control`, { action }),
    input: (id: string, action: HumanInputAction, params: Record<string, unknown>): Promise<unknown> => this.http.request('POST', `/api/control/sessions/${encodeURIComponent(id)}/input`, { action, params }),
    recover: (id: string, replace = false): Promise<unknown> => this.http.request('POST', `/api/control/sessions/${encodeURIComponent(id)}/recover`, { replace }),
    ticket: (id: string): Promise<{ ticket: string; expiresIn: number }> => this.http.request('POST', `/api/control/sessions/${encodeURIComponent(id)}/ticket`, {}),
    events: (after = 0): Promise<{ events: ControlEvent[]; cursor: number }> => this.http.request('GET', `/api/control/events?after=${after}`),
    createCredential: (options: { role: ControlRole; label?: string; expiresAt?: number }): Promise<ControlCredential & { token: string }> => this.http.request('POST', '/api/control/credentials', options),
    revokeCredential: (id: string): Promise<{ ok: boolean }> => this.http.request('DELETE', `/api/control/credentials/${encodeURIComponent(id)}`),
    members: (): Promise<{ owner: string | null; members: { userId: string; role: ControlRole }[] }> => this.http.request('GET', '/api/control/members'),
    inviteMember: (role: ControlRole = 'operator'): Promise<{ code: string; expiresIn: number }> => this.http.request('POST', '/api/control/members/invite', { role }),
    removeMember: (userId: string): Promise<{ ok: boolean }> => this.http.request('DELETE', `/api/control/members/${encodeURIComponent(userId)}`),
    createWebhook: (url: string, types: string[] = []): Promise<{ id: string; secret: string }> => this.http.request('POST', '/api/control/webhooks', { url, types }),
    removeWebhook: (id: string): Promise<{ ok: boolean }> => this.http.request('DELETE', `/api/control/webhooks/${encodeURIComponent(id)}`),
    replayDelivery: (id: string): Promise<{ ok: boolean }> => this.http.request('POST', `/api/control/deliveries/${encodeURIComponent(id)}/replay`, {}),
  };

  /** Playbooks saved with `browser.toPlaybook()`. */
  readonly playbooks = {
    list: async (): Promise<PlaybookSummary[]> =>
      (await this.http.request<{ playbooks: PlaybookSummary[] }>('GET', '/api/playbooks')).playbooks,
    /** Delete a playbook and its draft, or only the draft with `'<name>:draft'`. */
    remove: async (name: string): Promise<void> => { await this.http.request('DELETE', `/api/playbooks/${encodeURIComponent(name)}`); },
    /** Replace a playbook with the draft a healed replay saved. Try it first with `browser.play('<name>:draft')`. */
    promote: (name: string): Promise<Playbook> =>
      this.http.request<Playbook>('POST', `/api/playbooks/${encodeURIComponent(name)}/promote`, {}),
  };

  /**
   * Proxy exits for your personas. A persona takes one at first connect (by its
   * geo hint) or by `personas.pinProxy`, and keeps it.
   */
  readonly proxies = {
    list: async (): Promise<ProxyInfo[]> =>
      (await this.http.request<{ proxies: ProxyInfo[] }>('GET', '/api/proxies')).proxies,
    create: (proxy: ProxyCreate): Promise<ProxyInfo> => this.http.request<ProxyInfo>('POST', '/api/proxies', proxy),
    remove: async (id: string): Promise<void> => { await this.http.request('DELETE', `/api/proxies/${encodeURIComponent(id)}`); },
    /** Dial each proxy and learn its real exit IP. Failing ones cool down and are skipped. */
    check: async (): Promise<Array<{ id: string; ok: boolean; exitIp?: string | null; error?: string }>> =>
      (await this.http.request<{ results: Array<{ id: string; ok: boolean; exitIp?: string | null; error?: string }> }>('POST', '/api/proxies/check', {})).results,
  };

  readonly personas = {
    list: async (): Promise<PersonaInfo[]> =>
      (await this.http.request<{ personas: PersonaInfo[] }>('GET', '/api/personas')).personas,
    get: (id: string): Promise<PersonaInfo> => this.http.request<PersonaInfo>('GET', `/api/personas/${id}`),

    /**
     * Create an identity. The device — platform, timezone, locale — is chosen
     * here and fixed for its life; `preview()` shows what a choice produces.
     */
    create: (options: { name?: string; prefs?: PersonaPrefs; proxy?: { geo?: string }; maxConcurrent?: number | null } = {}): Promise<PersonaInfo> =>
      this.http.request<PersonaInfo>('POST', '/api/personas', options),

    /** Name, concurrency cap and proxy hint. Never the device — clone for that. */
    update: (id: string, changes: { name?: string; maxConcurrent?: number | null; proxy?: { geo?: string } | null }): Promise<PersonaInfo> =>
      this.http.request<PersonaInfo>('PUT', `/api/personas/${id}`, changes),

    /** A new persona of the same kind of device: same choices, fresh identity, empty jar. */
    clone: (id: string, options: { name?: string } = {}): Promise<PersonaInfo> =>
      this.http.request<PersonaInfo>('POST', `/api/personas/${id}/clone`, options),

    /** The fingerprint these choices would produce. Persists nothing. */
    preview: async (prefs: PersonaPrefs = {}): Promise<Fingerprint> =>
      (await this.http.request<{ fingerprint: Fingerprint }>('POST', '/api/personas/preview', { prefs })).fingerprint,

    /** Platforms, and the timezones and locales each may coherently claim. */
    options: (): Promise<{ platforms: string[]; timezones: Record<string, string[]>; locales: Record<string, string[]> }> =>
      this.http.request('GET', '/api/personas/options'),

    /** Pin the persona to one of your proxies, or `null` to let assignment happen at connect. */
    pinProxy: (id: string, proxyId: string | null) =>
      this.http.request<{ ok: boolean; proxy: { id: string; label: string } | null }>('PUT', `/api/personas/${id}/proxy`, { proxyId }),

    remove: async (id: string): Promise<void> => { await this.http.request('DELETE', `/api/personas/${id}`); },

    /** Store the second factor for this identity. Sealed at rest, never read back. */
    setMfa: (id: string, config: MfaConfig): Promise<{ configured: boolean; type: string }> =>
      this.http.request('PUT', `/api/personas/${id}/mfa`, config),

    clearMfa: async (id: string): Promise<void> => { await this.http.request('DELETE', `/api/personas/${id}/mfa`); },
  };

  /**
   * This key's settings: LLM credentials, browser provider, solver.
   *
   * Bring your own LLM key (it pays for its own tokens, so no hourly chat quota applies):
   *   await oya.config.set({ llm_provider: 'gemini', openai_api_key: process.env.GEMINI_API_KEY });
   * `llm_provider` is 'openai' | 'anthropic' | 'gemini' | 'vertex'; `chat_model` overrides
   * its default model. 'vertex' is Gemini Enterprise (ex-Vertex AI) in express mode, which
   * needs no GCP project; for a project-scoped endpoint, set `openai_base_url` to
   * `.../endpoints/openapi` and pass an OAuth access token as `openai_api_key`.
   */
  readonly config = {
    get: <T = Record<string, unknown>>(): Promise<T> => this.http.request<T>('GET', '/api/config'),
    set: <T = Record<string, unknown>>(values: Record<string, unknown>): Promise<T> =>
      this.http.request<T>('POST', '/api/config', values),
  };

  /** Saved profiles. `personas` is retained as an alias for existing clients. */
  readonly profiles = this.personas;

  usage(): Promise<unknown> { return this.http.request('GET', '/api/usage'); }

  private async waitUntilConnected(id: string, timeoutMs: number): Promise<void> {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const all = await this.browser.list();
      if (all.some((b) => b.id === id && b.health !== 'dead')) return;
      try {
        const session = await this.control.session(id);
        if (['failed', 'stopped', 'unknown_outcome'].includes(session.state)) throw new OyaError(`Browser creation ended in ${session.state}`, 409, session);
      } catch (e) { if (!(e instanceof OyaError) || e.status !== 404) throw e; }
      await new Promise((r) => setTimeout(r, READY_POLL_MS));
    }
    throw new OyaError(`Browser ${id} did not come up within ${Math.round(timeoutMs / 1000)}s`, 504, null);
  }
}

export default Oya;
