/**
 * @oya-ai/browser, thousands of browsers, one API.
 *
 *   import { Oya } from '@oya-ai/browser';
 *
 *   const oya = new Oya();                                    // OYA_API_KEY
 *   const browser = await oya.browser.start({ persona: 'auto', captcha: 'auto' });
 *   await browser.goto('https://example.com');
 *
 * Which provider actually runs the browser, Oya Cloud, your own machines,
 * Browser Use, Browserbase, Steel, Anchor, or a CDP URL you hand us, is
 * configuration on your API key, not something this code has to know.
 *
 * This file is the package's facade: the `Oya` client and everything exported.
 * Each namespace (`oya.browser`, `oya.control` …) is built in api/.
 */

import { createHttp, type Http } from './client.js';
import { Browser } from './browser.js';
import { Run } from './run.js';
import { OyaError } from './errors.js';
import { browserApi } from './api/browsers.js';
import { controlApi } from './api/control.js';
import { playbookApi } from './api/playbooks.js';
import { proxyApi } from './api/proxies.js';
import { personaApi } from './api/personas.js';
import { configApi } from './api/config.js';
import { waitUntilConnected } from './api/ready.js';
import { desktopApi, signup } from './api/agent.js';
import type { OyaOptions, Signup, SignupOptions } from './types/index.js';

export { Browser, Run, OyaError };
export { file, MAX_FILE_BYTES } from './file.js';
export * from './types/index.js';

/** The client: one API key, every browser behind it. */
export class Oya {
  /** The one HTTP path every call goes through. */
  private readonly http: Http;

  /** Reads the key and URL from `options`, then OYA_API_KEY and OYA_BASE_URL. Throws without a key. */
  constructor(options: OyaOptions = {}) {
    this.http = createHttp(options);
  }

  /** Start, reattach to, list and stop browsers. */
  readonly browser = browserApi(
    () => this.http,
    (id, timeoutMs) => this.waitUntilConnected(id, timeoutMs),
  );

  /** The person's own desktop browser, with their logins: `await oya.desktop.connect()` pairs it when it is not up yet. */
  readonly desktop = desktopApi(
    () => this.http,
    () => this.browser.list(),
  );

  /**
   * An agent's own key, with no person or dashboard: `await Oya.signup({ email })`.
   * Saved to ~/.oya/config.json, so a later `new Oya()` finds it.
   */
  static signup(options: SignupOptions): Promise<Signup> {
    return signup(options);
  }

  /** Durable operational controls, including disconnected and cleanup-pending sessions. */
  readonly control = controlApi(() => this.http);

  /** Playbooks saved with `browser.toPlaybook()`. */
  readonly playbooks = playbookApi(() => this.http);

  /**
   * Proxy exits for your personas. A persona takes one at first connect (by its
   * geo hint) or by `personas.pinProxy`, and keeps it.
   */
  readonly proxies = proxyApi(() => this.http);

  /** Identities: fingerprint, cookie jar and proxy bound together, plus their stored factors and logins. */
  readonly personas = personaApi(() => this.http);

  /**
   * This key's settings: LLM credentials, browser provider, solver.
   *
   * Bring your own LLM key (it pays for its own tokens, so no hourly chat quota applies):
   *   await oya.config.set({ llm_provider: 'gemini', openai_api_key: process.env.GEMINI_API_KEY });
   * `llm_provider` is the {@link LlmProvider} union, so an editor offers the choices and a
   * typo is a compile error; `chat_model` overrides its default model. 'vertex' is Gemini
   * Enterprise (ex-Vertex AI) in express mode, which needs no GCP project; for a
   * project-scoped endpoint, set `openai_base_url` to `.../endpoints/openapi` and pass an
   * OAuth access token as `openai_api_key`. See {@link ConfigUpdate} for every field.
   */
  readonly config = configApi(() => this.http);

  /** Saved profiles. `personas` is retained as an alias for existing clients. */
  readonly profiles = this.personas;

  /** What this key has spent. */
  usage(): Promise<unknown> {
    return this.http.request('GET', '/api/usage');
  }

  /** Waits for a starting browser to dial in, through this client's own list and session calls. */
  private async waitUntilConnected(id: string, timeoutMs: number): Promise<void> {
    const checks = { list: () => this.browser.list(), session: (sid: string) => this.control.session(sid) };
    await waitUntilConnected(checks, id, timeoutMs);
  }
}

export default Oya;
