/**
 * Browser: one running browser, driven through the API. Page actions go
 * through the command endpoint; CAPTCHA, MFA, agent runs, playbooks and live
 * view links have their own endpoints. The small rules the methods share
 * (element ids, aimed scrolls, agent errors) are the functions below the class.
 */
import type { Http } from './client.js';
import type {
  Analysis,
  BrowserDetail,
  CaptchaResult,
  Element,
  MfaResult,
  Playbook,
  PlayResult,
  RunData,
  RunInfo,
  StartResult,
  StopResult,
  SubmitOptions,
} from './types/index.js';
import type {
  AgentText,
  AskValues,
  CommandResult,
  OpenTabData,
  DialogResult,
  PlayOptions,
  ScreenshotData,
  Point,
  ShareLink,
  ShareOptions,
  Tab,
  TabsData,
  Task,
  TicketData,
  TypeResult,
} from './browser-shapes.js';
import { OyaError } from './errors.js';
import { Run } from './run.js';
import {
  AGENT_TIMEOUT_MS,
  AIMED_SCROLL_AMOUNT,
  CHALLENGE_TIMEOUT_MS,
  NAVIGATE_TIMEOUT_MS,
  PLAYBOOK_TIMEOUT_MS,
  Status,
  STOP_TIMEOUT_MS,
  WAIT_FOR_DEFAULT_MS,
  WAIT_FOR_GRACE_MS,
} from './constants.js';

/**
 * One running browser.
 *
 * Element ids come from `analyze()` and are only valid until the page changes —
 * the same contract the agent tools use. `click(13)` after a navigation is a
 * bug; call `analyze()` again.
 */
// The public surface is fixed by the published API: every method below is a
// documented one- to three-line call, and splitting the class would change the
// emitted types. See packages/ARCHITECTURE.md, Exceptions.
// eslint-disable-next-line local/max-class-lines
export class Browser {
  /** The browser's id. */
  readonly id: string;
  /** Which provider runs it. */
  readonly provider: string;
  /** Which persona it runs as. */
  readonly persona: string;
  /** Point Playwright, Puppeteer or browser-use here. */
  readonly cdpUrl?: string;

  /** Wraps a started browser; `autoCaptcha` solves CAPTCHAs after every `goto()`. */
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

  /** Runs one browser command; a command that ran and failed throws. */
  private async command<T>(action: string, params: Record<string, unknown> = {}, timeoutMs?: number): Promise<T> {
    const path = `/api/browsers/${this.id}/command`;
    const result = await this.http.request<CommandResult<T>>('POST', path, { action, params }, timeoutMs);
    if (result.ok === false) throw new OyaError(result.error || `${action} failed`, Status.UNPROCESSABLE, result);
    return result.data as T;
  }

  /** Navigates, then clears any CAPTCHA when `captcha: 'auto'` was asked for. */
  async goto(url: string): Promise<void> {
    await this.command('navigate', { url }, NAVIGATE_TIMEOUT_MS);
    if (this.autoCaptcha) assertCaptchaCleared(await this.solveCaptcha());
  }

  /** The page as markdown plus numbered elements to act on. */
  async analyze(): Promise<Analysis> {
    return this.command<Analysis>('analyze');
  }

  /** Only the visible elements, which is what an agent almost always wants. */
  async elements(): Promise<Element[]> {
    return (await this.analyze()).elements.filter((e) => e.visible);
  }

  /** Clicks an element by its id from `analyze()`. */
  async click(elementId: number | string): Promise<void> {
    const id = this.elementId(elementId);
    await this.command('click', { element_id: id, selector: `[data-ac-id="${id}"]` });
  }

  /** Types into an element by its id from `analyze()`. */
  async type(elementId: number | string, text: string): Promise<TypeResult> {
    const id = this.elementId(elementId);
    return this.command('type', { element_id: id, selector: `[data-ac-id="${id}"]`, text });
  }

  /** A valid element id, or an OyaError saying where ids come from. */
  private elementId(value: number | string): number {
    return toElementId(value);
  }

  /** Presses one key, such as Enter or Escape. */
  async pressKey(key: string): Promise<void> {
    await this.command('press_key', { key });
  }

  /**
   * Answer a native dialog holding the page. alert() and beforeunload are
   * answered for you; a confirm() or prompt() waits for this, and every other
   * command fails fast with the dialog's text until it is answered.
   */
  async handleDialog(accept: boolean, promptText?: string): Promise<DialogResult> {
    return this.command('handle_dialog', { accept, prompt_text: promptText });
  }

  /** `at` aims the wheel at an inner scroller (a results panel, a chat pane) instead of the page. */
  async scroll(direction: 'up' | 'down' | 'top' | 'bottom', amount?: number, at?: Point): Promise<void> {
    await this.command('scroll', scrollParams(direction, amount, at));
  }

  /** Waits until `selector` matches, up to `timeout` milliseconds. */
  async waitFor(selector: string, timeout = WAIT_FOR_DEFAULT_MS): Promise<void> {
    await this.command('wait', { selector, timeout }, timeout + WAIT_FOR_GRACE_MS);
  }

  /** A `data:image/…;base64,` URL. PNG or JPEG depending on the driver. */
  async screenshot(): Promise<string> {
    return (await this.command<ScreenshotData>('screenshot')).screenshot;
  }

  /** The active tab's URL, or empty when there is none. */
  async url(): Promise<string> {
    return (await this.tabs()).find((t) => t.active)?.url || '';
  }

  /** Every open tab. */
  async tabs(): Promise<Array<Tab>> {
    return (await this.command<TabsData>('list_tabs')).tabs || [];
  }

  /** Opens a tab, optionally at `url`, and returns its id. */
  async openTab(url?: string): Promise<string> {
    return (await this.command<OpenTabData>('open_tab', { url }, NAVIGATE_TIMEOUT_MS)).tab_id;
  }

  /** Makes a tab the one commands act on. */
  async switchTab(tabId: string): Promise<void> {
    await this.command('switch_tab', { tab_id: tabId });
  }

  /** Closes a tab. */
  async closeTab(tabId: string): Promise<void> {
    await this.command('close_tab', { tab_id: tabId });
  }

  /**
   * Detect and clear a CAPTCHA. Providers that solve natively are left to do
   * it; everything else goes to the configured solver.
   */
  solveCaptcha(): Promise<CaptchaResult> {
    return this.http.request<CaptchaResult>('POST', `/api/browsers/${this.id}/captcha`, {}, CHALLENGE_TIMEOUT_MS);
  }

  /**
   * Answer an MFA prompt with the persona's configured factor. When nothing can
   * answer it, `liveViewUrl` is where a person finishes by hand.
   */
  async completeMfa(): Promise<MfaResult> {
    const result = await this.http.request<MfaResult>('POST', `/api/browsers/${this.id}/mfa`, {}, CHALLENGE_TIMEOUT_MS);
    if (result.liveViewUrl) result.liveViewUrl = new URL(result.liveViewUrl, this.http.baseUrl).href;
    return result;
  }

  /**
   * Natural-language control, using this key's configured model. Refer to values as
   * `{{name}}` in the prompt. `data` the agent can read, so it can split a name or pick
   * the matching option; `secrets` it never sees. It types both through placeholders,
   * with filters like `{{name|first}}`, so a playbook saved from the run stores no values.
   */
  async ask(prompt: string, { data, secrets }: AskValues = {}): Promise<string> {
    const body = { messages: [{ role: 'user', content: prompt }], data, secrets };
    const path = `/api/browsers/${this.id}/chat`;
    return agentAnswer(await this.http.request<AgentAnswer<AgentText>>('POST', path, body, AGENT_TIMEOUT_MS)).text;
  }

  /**
   * Save the last `ask()` on this browser as a named playbook. Every value that was
   * typed, picked or clicked becomes a variable, with what the run used kept in
   * `defaults`, so `play()` with nothing repeats the run and any one value can be
   * swapped. `code` is the same flow as a Playwright module, to read or run yourself.
   */
  toPlaybook(name: string): Promise<Playbook> {
    return this.http.request<Playbook>('POST', `/api/browsers/${this.id}/playbooks`, { name }, PLAYBOOK_TIMEOUT_MS);
  }

  /**
   * Replay a playbook with no LLM in the loop. Variables left out reuse the
   * recorded values where there are any. If a step no longer fits the page and
   * `autoHeal` is on (the default), the agent finishes the task and its fix is
   * saved as a draft (`healed`, `draft`); off, the step's error is thrown.
   * Play `'<name>:draft'` to try a draft before promoting it.
   */
  async play(name: string, data: RunData = {}, { autoHeal = true }: PlayOptions = {}): Promise<PlayResult> {
    const path = `/api/browsers/${this.id}/playbooks/${encodeURIComponent(name)}/play`;
    return agentAnswer<PlayResult>(
      await this.http.request<AgentAnswer<PlayResult>>('POST', path, { variables: data, autoHeal }, AGENT_TIMEOUT_MS),
    );
  }

  /**
   * Start a prompt or playbook in the background and hear back through callbacks.
   * `onHumanAttention` fires for an unsolved CAPTCHA, an unfinished MFA, the agent
   * asking for help, or a replay the agent could not heal; the run waits (up to 30
   * minutes) until you call `respond()`.
   */
  async submit(task: Task, options: SubmitOptions = {}): Promise<Run> {
    const { onSuccess, onFailure, onHumanAttention, onHealed, pollMs, ...body } = options;
    const started = await this.http.request<RunInfo>('POST', `/api/browsers/${this.id}/runs`, { ...task, ...body });
    return new Run(this.http, started.id, { onSuccess, onFailure, onHumanAttention, onHealed }, pollMs);
  }

  /**
   * Watch it work: the console, opened on this browser. It carries no
   * credential. For the frames themselves, use `liveStreamUrl()`.
   */
  liveViewUrl(): string {
    return consoleLink(this.http.baseUrl, this.id);
  }

  /**
   * The SSE stream of JPEG frames, for embedding in your own UI. EventSource
   * cannot set headers, so the URL carries a connection ticket: single use,
   * 60 seconds. Mint one per viewer — the first connection spends it.
   */
  async liveStreamUrl(): Promise<string> {
    const path = `/api/control/sessions/${encodeURIComponent(this.id)}/ticket`;
    const { ticket } = await this.http.request<TicketData>('POST', path, {});
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
  async shareUrl({ control = false, expiresInSeconds = 3600 }: ShareOptions = {}): Promise<ShareLink> {
    const path = `/api/control/sessions/${encodeURIComponent(this.id)}/share`;
    const c = await this.http.request<ShareCredential>('POST', path, { control, expiresIn: expiresInSeconds });
    return { url: shareLink(this.http.baseUrl, this.id, c.token), id: c.id, expiresAt: c.expiresAt };
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
    return this.http.request<StopResult>('POST', `/api/browsers/${this.id}/stop`, {}, STOP_TIMEOUT_MS);
  }

  /** `await using browser = await oya.browser.start()` stops it however the block exits, errors included. */
  async [Symbol.asyncDispose](): Promise<void> {
    await this.stop();
  }

  /** @deprecated use stop() — close() only dropped the socket, and a cloud browser redialled. */
  async close(): Promise<void> {
    await this.stop();
  }
}

/** The credential the share endpoint mints. */
interface ShareCredential {
  /** Its id, for revoking it. */
  id: string;
  /** The secret the link carries. */
  token: string;
  /** When it stops working. */
  expiresAt: number | null;
}

/** An agent endpoint's answer, which may carry a failure in its body. */
type AgentAnswer<T> = T & {
  /** Why the run failed. */
  error?: string;
  /** The failure's HTTP status. */
  status?: number;
};

/** After `captcha: 'auto'`: a CAPTCHA still on screen needs a person. */
function assertCaptchaCleared(result: CaptchaResult): void {
  if (!result.present || result.solved || result.invisible) return;
  const message = result.error || 'CAPTCHA needs attention. Call solveCaptcha() again or open the live view.';
  throw new OyaError(message, Status.CONFLICT, result);
}

/** A non-negative whole number, from a number or a numeric string. */
function toElementId(value: number | string): number {
  const id = Number(value);
  const wrongType = typeof value !== 'number' && typeof value !== 'string';
  if (wrongType || value === '' || !Number.isInteger(id) || id < 0) {
    throw new OyaError('Use a numeric element id from browser.analyze().', Status.BAD_REQUEST, null);
  }
  return id;
}

/** An aimed scroll is one wheel event at that point; drivers only honour x/y in that mode. */
function scrollParams(direction: string, amount: number | undefined, at: Point | undefined): Record<string, unknown> {
  if (!at) return { direction, amount };
  return { direction, amount: amount ?? AIMED_SCROLL_AMOUNT, ...at, smooth: false };
}

/** The server sends 200 up front to keep long runs alive, so failures arrive in the body. */
function agentAnswer<T>(res: AgentAnswer<T>): AgentAnswer<T> {
  if (res.error) throw new OyaError(res.error, res.status ?? Status.SERVER_ERROR, res);
  return res;
}

/**
 * The console deep link for a browser. This used to return the raw frame stream
 * with the project's API key in the query string — a permanent credential in
 * browser history, Referer headers and every proxy log on the way, and a URL that
 * renders as a wall of text/event-stream if a person actually opens it. It is the
 * console deep link now, the same one the server hands back from `completeMfa()`,
 * and it carries no credential at all.
 */
function consoleLink(baseUrl: string, id: string): string {
  return `${baseUrl}/dashboard/?browser=${encodeURIComponent(id)}`;
}

/** The public live-view URL, with the token in the fragment so no server log sees it. */
function shareLink(baseUrl: string, id: string, token: string): string {
  return `${baseUrl}/live/${encodeURIComponent(id)}#t=${encodeURIComponent(token)}`;
}
