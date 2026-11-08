/** Only an app-started, window-owned login may pair without a second confirmation dialog. */
import { randomBytes } from 'node:crypto';
import type { WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import { pairingServer } from './pairing.ts';
import { readApiAnswer, serverHttpBase } from './server-api.ts';
import { desktopCallback } from './desktop-callback.ts';
import { openOutside } from '../chatgpt/index.ts';
import * as C from './constants.ts';

/** Authentication owns only these services and cannot import browser data. */
type Deps = Pick<AppServices, 'config' | 'electron' | 'deepLinks' | 'socket' | 'modelSettings'>;
/** An attempt retains the original workspace and an explicit cancellation owner. */
interface Attempt {
  /** Identity of the exact initiating shell contents. */ owner: WebContents;
  /** Captured destination, never accepted from a callback. */ serverUrl: string;
  /** A concurrent project change invalidates this login. */ apiKey: string;
  /** Closes listener and network request on every exit. */ abort: AbortController;
}
/** One bounded attempt per window shares the application's workspace ownership checks. */
export class DesktopSignIn {
  /** Main-owned configuration, pairing and socket services. */ private readonly deps: Deps;
  /** Listener seam keeps ownership tests hermetic without binding a port. */ private readonly callback: typeof desktopCallback;
  /** Live attempts are discarded after completion, cancellation or owner closure. */ private readonly attempts =
    new Map<number, Attempt>();
  /** Compose this boundary once, rather than trusting renderer URLs or executable paths. */
  constructor(deps: Deps, callback = desktopCallback) {
    this.deps = deps;
    this.callback = callback;
  }
  /** Start from the configured server and keep the callback secret within this attempt. */
  async start(owner: WebContents): Promise<void> {
    this.cancel(owner.id);
    const attempt = this.create(owner);
    await this.owned(attempt);
  }
  /** A window closing invalidates its attempt before any later network answer. */
  private async owned(attempt: Attempt): Promise<void> {
    const owner = attempt.owner;
    const closed = () => this.cancel(owner.id);
    owner.once('destroyed', closed);
    try {
      await this.run(attempt);
    } finally {
      this.release(attempt, closed);
    }
  }
  /** Replaced attempts cannot cancel a newer login from the same window. */
  private release(attempt: Attempt, closed: () => void): void {
    attempt.owner.removeListener('destroyed', closed);
    if (this.attempts.get(attempt.owner.id) === attempt) this.cancel(attempt.owner.id);
  }
  /** Only safe WebSocket destinations may initiate authentication. */
  private create(owner: WebContents): Attempt {
    const { serverUrl, apiKey } = this.deps.config.values;
    const parsed = pairingServer(serverUrl);
    if (!parsed || parsed.username || parsed.password || owner.isDestroyed())
      throw Error('Check your server address and try again.');
    const attempt = { owner, serverUrl, apiKey, abort: new AbortController() };
    this.attempts.set(owner.id, attempt);
    return attempt;
  }
  /** Expiry aborts the underlying listener, opener wait and pairing request. */
  private async run(attempt: Attempt): Promise<void> {
    const timer = setTimeout(() => attempt.abort.abort(), C.DESKTOP_LOGIN_TTL_MS);
    try {
      await this.authorize(attempt);
    } finally {
      clearTimeout(timer);
      attempt.abort.abort();
    }
  }
  /** Only a validated loopback code reaches the captured HTTPS pairing endpoint. */
  private async authorize(attempt: Attempt): Promise<void> {
    const state = randomBytes(C.DESKTOP_STATE_BYTES).toString('hex');
    const code = await this.receive(attempt, state);
    this.check(attempt);
    await this.claim(attempt, code);
  }
  /** The listener is always closed, including failed OS launch and timeout. */
  private async receive(attempt: Attempt, state: string): Promise<string> {
    const callback = await this.callback(state, attempt.abort.signal);
    try {
      const url = loginUrl(attempt.serverUrl, callback.url, state);
      // Never back into Oya itself: as the default browser it would hide the setup screen
      // and show the dashboard in a persona with no console session.
      await opened(openOutside(this.deps, url), attempt.abort.signal);
      return await callback.done;
    } finally {
      callback.close();
    }
  }
  /** Redeem once, then recheck ownership before any shared configuration changes. */
  private async claim(attempt: Attempt, code: string): Promise<void> {
    const answer = await claimCode(attempt, code);
    this.check(attempt);
    if (typeof answer.apiKey !== 'string' || !answer.apiKey) throw Error('Sign-in didn’t finish. Try again.');
    this.deps.modelSettings?.adoptLogin(attempt.owner.id, String(answer.apiKey));
    this.deps.deepLinks.retarget(pairedTarget(attempt.serverUrl, answer));
    this.deps.socket.connect();
  }
  /** A closed window, replaced attempt or changed workspace may never consume a late return. */
  private check(attempt: Attempt): void {
    attempt.abort.signal.throwIfAborted();
    const config = this.deps.config.values;
    if (
      attempt.owner.isDestroyed() ||
      this.attempts.get(attempt.owner.id) !== attempt ||
      !sameWorkspace(config, attempt)
    )
      throw Error('Your workspace changed. Start connecting again.');
  }
  /** Cancel one owner; sign-out and shutdown cancel every owner. */
  cancel(owner?: number): void {
    for (const [id, attempt] of this.attempts)
      if (owner === undefined || owner === id) {
        this.attempts.delete(id);
        attempt.abort.abort();
      }
  }
}
/** The return destination contains no API key and is bounded by native validation. */
function loginUrl(server: string, callback: string, state: string): string {
  const url = new URL('/dashboard', serverHttpBase(server));
  url.search = new URLSearchParams({ connect: 'desktop', callback, state }).toString();
  return url.href;
}
/** An OS opener that never settles cannot outlive its owning authentication attempt. */
function opened(work: Promise<void>, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const abort = () => reject(Error('Sign-in didn’t finish. Try again.'));
    signal.addEventListener('abort', abort, { once: true });
    void work.then(resolve, reject).finally(() => signal.removeEventListener('abort', abort));
    if (signal.aborted) abort();
  });
}

/** Snapshot the code exchange destination and abort the underlying request on cancellation. */
async function claimCode(attempt: Attempt, code: string): Promise<Record<string, unknown>> {
  const signal = AbortSignal.any([attempt.abort.signal, AbortSignal.timeout(C.PAIRING_TIMEOUT_MS)]);
  const init = claimRequest(code, signal);
  return (await readApiAnswer(await fetch(`${serverHttpBase(attempt.serverUrl)}/api/pairing/claim`, init))) as Record<
    string,
    unknown
  >;
}
/** Shared workspace changes invalidate other windows' in-flight login attempts. */
const sameWorkspace = (config: Pick<Attempt, 'apiKey' | 'serverUrl'>, attempt: Attempt) =>
  config.apiKey === attempt.apiKey && config.serverUrl === attempt.serverUrl;

/** Only credential fields returned by the captured server are applied to the workspace. */
function pairedTarget(serverUrl: string, answer: Record<string, unknown>) {
  return {
    serverUrl,
    apiKey: String(answer.apiKey),
    persona: typeof answer.persona === 'string' ? answer.persona : 'default',
  };
}
/** A one-shot claim never follows redirects with its code. */
function claimRequest(code: string, signal: AbortSignal): RequestInit {
  return {
    method: 'POST',
    redirect: 'error',
    signal,
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ code }),
  };
}
