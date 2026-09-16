import type { Http } from './client.js';
import type {
  Analysis, BrowserDetail, CaptchaResult, Element, MfaResult, Playbook, PlayResult, RunData, RunInfo, RunResult, StartResult, StopResult, SubmitOptions,
} from './types.js';
import { OyaError } from './types.js';

/** Navigation is slow and the server disables its own timeout for it. */
const NAVIGATE_TIMEOUT_MS = 120_000;

interface CommandResult<T = unknown> {
  ok: boolean;
  data?: T;
  error?: string;
}

/**
 * One running browser.
 *
 * Element ids come from `analyze()` and are only valid until the page changes —
 * the same contract the agent tools use. `click(13)` after a navigation is a
 * bug; call `analyze()` again.
 */
export class Browser {
  readonly id: string;
  readonly provider: string;
  readonly persona: string;
  /** Point Playwright, Puppeteer or browser-use here. */
  readonly cdpUrl?: string;

  constructor(
    private readonly http: Http,
    info: StartResult,
    private readonly autoCaptcha: boolean,
  ) {
    this.id = info.id;
    this.provider = info.provider;
    this.persona = info.persona;
    this.cdpUrl = info.cdpUrl;
  }

  private async command<T>(action: string, params: Record<string, unknown> = {}, timeoutMs?: number): Promise<T> {
    const result = await this.http.request<CommandResult<T>>(
      'POST', `/api/browsers/${this.id}/command`, { action, params }, timeoutMs);
    if (result.ok === false) throw new OyaError(result.error || `${action} failed`, 422, result);
    return result.data as T;
  }

  async goto(url: string): Promise<void> {
    await this.command('navigate', { url }, NAVIGATE_TIMEOUT_MS);
    if (this.autoCaptcha) {
      const result = await this.solveCaptcha();
      if (result.present && !result.solved && !result.invisible) throw new OyaError(result.error || 'CAPTCHA needs attention. Call solveCaptcha() again or open the live view.', 409, result);
    }
  }

  /** The page as markdown plus numbered elements to act on. */
  async analyze(): Promise<Analysis> {
    return this.command<Analysis>('analyze');
  }

  /** Only the visible elements, which is what an agent almost always wants. */
  async elements(): Promise<Element[]> {
    return (await this.analyze()).elements.filter((e) => e.visible);
  }

  async click(elementId: number | string): Promise<void> {
    const id = this.elementId(elementId);
    await this.command('click', { element_id: id, selector: `[data-ac-id="${id}"]` });
  }

  async type(elementId: number | string, text: string): Promise<{ suggestions_visible?: boolean }> {
    const id = this.elementId(elementId);
    return this.command('type', { element_id: id, selector: `[data-ac-id="${id}"]`, text });
  }

  private elementId(value: number | string): number {
    const id = Number(value);
    if ((typeof value !== 'number' && typeof value !== 'string') || value === '' || !Number.isInteger(id) || id < 0) {
      throw new OyaError('Use a numeric element id from browser.analyze().', 400, null);
    }
    return id;
  }

  async pressKey(key: string): Promise<void> {
    await this.command('press_key', { key });
  }

  /** `at` aims the wheel at an inner scroller (a results panel, a chat pane) instead of the page. */
  async scroll(direction: 'up' | 'down' | 'top' | 'bottom', amount?: number, at?: { x: number; y: number }): Promise<void> {
    // An aimed scroll is one wheel event at that point; drivers only honour x/y in that mode.
    await this.command('scroll', at ? { direction, amount: amount ?? 500, ...at, smooth: false } : { direction, amount });
  }

  async waitFor(selector: string, timeout = 30_000): Promise<void> {
    await this.command('wait', { selector, timeout }, timeout + 5_000);
  }

  /** A `data:image/…;base64,` URL. PNG or JPEG depending on the driver. */
  async screenshot(): Promise<string> {
    const data = await this.command<{ screenshot: string }>('screenshot');
    return data.screenshot;
  }

  async url(): Promise<string> {
    const tabs = await this.tabs();
    return tabs.find((t) => t.active)?.url || '';
  }

  async tabs(): Promise<Array<{ id: string; url: string; title: string; active: boolean }>> {
    const data = await this.command<{ tabs: Array<{ id: string; url: string; title: string; active: boolean }> }>('list_tabs');
    return data.tabs || [];
  }

  async openTab(url?: string): Promise<string> {
    const data = await this.command<{ tab_id: string }>('open_tab', { url }, NAVIGATE_TIMEOUT_MS);
    return data.tab_id;
  }

  async switchTab(tabId: string): Promise<void> { 
    await this.command('switch_tab', { tab_id: tabId }); 
  }

  async closeTab(tabId: string): Promise<void> { 
    await this.command('close_tab', { tab_id: tabId }); 
  }

  /**
  x * Detect and clear a CAPTCHA. Providers that solve natively are left to do
   * it; everything else goes to the configured solver.
   */
  solveCaptcha(): Promise<CaptchaResult> {
    return this.http.request<CaptchaResult>('POST', `/api/browsers/${this.id}/captcha`, {}, 180_000);
  }

  /**
   * Answer an MFA prompt with the persona's configured factor. When nothing can
   * answer it, `liveViewUrl` is where a person finishes by hand.
   */
  async completeMfa(): Promise<MfaResult> {
    const result = await this.http.request<MfaResult>('POST', `/api/browsers/${this.id}/mfa`, {}, 180_000);
    if (result.liveViewUrl) result.liveViewUrl = new URL(result.liveViewUrl, this.http.baseUrl).href;
    return result;
  }

  /**
   * Natural-language control, using this key's configured model. Refer to values as
   * `{{name}}` in the prompt. `data` the agent can read, so it can split a name or pick
   * the matching option; `secrets` it never sees. It types both through placeholders,
   * with filters like `{{name|first}}`, so a playbook saved from the run stores no values.
   */
  async ask(prompt: string, { data, secrets }: { data?: RunData; secrets?: RunData } = {}): Promise<string> {
    const res = await this.http.request<{ text: string; error?: string; status?: number }>(
      'POST', `/api/browsers/${this.id}/chat`, { messages: [{ role: 'user', content: prompt }], data, secrets }, 600_000);
    // The server sends 200 up front to keep long runs alive, so failures arrive in the body.
    if (res.error) throw new OyaError(res.error, res.status ?? 500, res);
    return res.text;
  }

  /**
   * Save the last `ask()` on this browser as a named playbook. Values that came
   * from the prompt (names, IDs, dates) become variables; `code` is the same
   * flow as a Playwright module, to read or run yourself.
   */
  toPlaybook(name: string): Promise<Playbook> {
    return this.http.request<Playbook>('POST', `/api/browsers/${this.id}/playbooks`, { name }, 120_000);
  }

  /**
   * Replay a playbook with no LLM in the loop. Variables left out reuse the
   * recorded values where there are any. If a step no longer fits the page and
   * `autoHeal` is on (the default), the agent finishes the task and its fix is
   * saved as a draft (`healed`, `draft`); off, the step's error is thrown.
   * Play `'<name>:draft'` to try a draft before promoting it.
   */
  async play(name: string, data: RunData = {}, { autoHeal = true }: { autoHeal?: boolean } = {}): Promise<PlayResult> {
    const res = await this.http.request<PlayResult & { error?: string; status?: number }>(
      'POST', `/api/browsers/${this.id}/playbooks/${encodeURIComponent(name)}/play`, { variables: data, autoHeal }, 600_000);
    if (res.error) throw new OyaError(res.error, res.status ?? 500, res);
    return res;
  }

  /**
   * Start a prompt or playbook in the background and hear back through callbacks.
   * `onHumanAttention` fires for an unsolved CAPTCHA, an unfinished MFA, the agent
   * asking for help, or a replay the agent could not heal; the run waits (up to 30
   * minutes) until you call `respond()`.
   */
  async submit(task: { prompt: string } | { playbook: string }, options: SubmitOptions = {}): Promise<Run> {
    const { onSuccess, onFailure, onHumanAttention, onHealed, pollMs, ...body } = options;
    const started = await this.http.request<RunInfo>('POST', `/api/browsers/${this.id}/runs`, { ...task, ...body });
    return new Run(this.http, started.id, { onSuccess, onFailure, onHumanAttention, onHealed }, pollMs);
  }

  /**
   * Watch it work: the console, opened on this browser.
   *
   * This used to return the raw frame stream with the project's API key in the
   * query string — a permanent credential in browser history, Referer headers
   * and every proxy log on the way, and a URL that renders as a wall of
   * text/event-stream if a person actually opens it. It is the console deep
   * link now, the same one the server hands back from `completeMfa()`, and it
   * carries no credential at all.
   *
   * For the frames themselves, use `liveStreamUrl()`.
   */
  liveViewUrl(): string {
    return `${this.http.baseUrl}/dashboard/?browser=${encodeURIComponent(this.id)}`;
  }

  /**
   * The SSE stream of JPEG frames, for embedding in your own UI. EventSource
   * cannot set headers, so the URL carries a connection ticket: single use,
   * 60 seconds. Mint one per viewer — the first connection spends it.
   */
  async liveStreamUrl(): Promise<string> {
    const { ticket } = await this.http.request<{ ticket: string }>(
      'POST', `/api/control/sessions/${encodeURIComponent(this.id)}/ticket`, {});
    return `${this.http.baseUrl}/api/live/${this.id}?ticket=${encodeURIComponent(ticket)}`;
  }

  /**
   * A shareable link to this browser's live view, for handing to a person or
   * embedding in your own app. The token rides in the URL fragment, so it never
   * reaches a server log or Referer header. `control: true` lets whoever opens
   * it take over and act in the browser; otherwise it is view-only. The link
   * expires (default one hour) and is revocable with `revokeShare(id)`.
   *
   * The credential is scoped to this one browser: it cannot see or touch the
   * rest of your project. Anyone holding the link has that access until it
   * expires or you revoke it, so treat it like a password.
   */
  async shareUrl(
    { control = false, expiresInSeconds = 3600 }: { control?: boolean; expiresInSeconds?: number } = {},
  ): Promise<{ url: string; id: string; expiresAt: number | null }> {
    const c = await this.http.request<{ id: string; token: string; expiresAt: number | null }>(
      'POST', `/api/control/sessions/${encodeURIComponent(this.id)}/share`, { control, expiresIn: expiresInSeconds });
    return { url: `${this.http.baseUrl}/live/${encodeURIComponent(this.id)}#t=${encodeURIComponent(c.token)}`, id: c.id, expiresAt: c.expiresAt };
  }

  /** Revoke a link from `shareUrl()` before it expires, by the id it returned. */
  async revokeShare(id: string): Promise<void> {
    await this.http.request('DELETE', `/api/control/credentials/${encodeURIComponent(id)}`);
  }

  /** Counters, health and the last 50 things this browser did. */
  status(): Promise<BrowserDetail> {
    return this.http.request<BrowserDetail>('GET', `/api/browsers/${this.id}`);
  }

  /**
   * Stop it, whatever it is: a cloud sandbox is destroyed so billing ends, a
   * CDP session is handed back to its provider, a desktop browser disconnects.
   */
  stop(): Promise<StopResult> {
    return this.http.request<StopResult>('POST', `/api/browsers/${this.id}/stop`, {}, 60_000);
  }

  /** `await using browser = await oya.browser.start()` stops it however the block exits, errors included. */
  async [Symbol.asyncDispose](): Promise<void> { await this.stop(); }

  /** @deprecated use stop() — close() only dropped the socket, and a cloud browser redialled. */
  async close(): Promise<void> { await this.stop(); }
}

type RunCallbacks = Pick<SubmitOptions, 'onSuccess' | 'onFailure' | 'onHumanAttention' | 'onHealed'>;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** A submitted task. Callbacks fire as it changes; `done` settles when it ends. */
export class Run {
  readonly done: Promise<RunResult>;

  constructor(private readonly http: Http, readonly id: string, callbacks: RunCallbacks, pollMs = 2_000) {
    this.done = this.watch(callbacks, pollMs);
    this.done.catch(() => {}); // callers may rely on onFailure alone
  }

  status(): Promise<RunInfo> {
    return this.http.request<RunInfo>('GET', `/api/runs/${encodeURIComponent(this.id)}`);
  }

  /** Answer the open attention request: `'done'` after handling it by hand, or your reply to the agent. */
  async respond(response = 'done'): Promise<void> {
    await this.http.request('POST', `/api/runs/${encodeURIComponent(this.id)}/respond`, { response });
  }

  private async watch(cb: RunCallbacks, pollMs: number): Promise<RunResult> {
    const call = async (fn: () => unknown) => {
      try { await fn(); } catch (err) { console.error('[oya] run callback threw:', err); }
    };
    let seen: string | undefined;
    for (let errors = 0; ;) {
      let run: RunInfo;
      try {
        run = await this.status();
        errors = 0;
      } catch (err) {
        if (++errors < 5) { await sleep(pollMs); continue; }
        const failure = err instanceof OyaError ? err : new OyaError(String(err), 0, null);
        await call(() => cb.onFailure?.(failure));
        throw failure;
      }

      if (run.status === 'needs_attention' && run.attention && run.attention.id !== seen) {
        seen = run.attention.id;
        const request = {
          ...run.attention,
          liveViewUrl: run.attention.liveViewUrl && new URL(run.attention.liveViewUrl, this.http.baseUrl).href,
          respond: (response?: string) => this.respond(response),
        };
        // Not awaited: a handler that waits on a person must not stall polling.
        void call(() => cb.onHumanAttention?.(request));
      }
      if (run.status === 'succeeded') {
        const result = run.result || {};
        if (result.healed) await call(() => cb.onHealed?.(result));
        await call(() => cb.onSuccess?.(result));
        return result;
      }
      if (run.status === 'failed') {
        const failure = new OyaError(run.error || 'Run failed', run.errorStatus ?? 500, run);
        await call(() => cb.onFailure?.(failure));
        throw failure;
      }
      await sleep(pollMs);
    }
  }
}
