/**
 * What an agent needs to get going on its own: a key of its own
 * (`Oya.signup`, which solves the server's proof-of-work puzzle so no agent
 * has to), and the person's own desktop browser with their logins
 * (`oya.desktop.connect`, which pairs the desktop app and waits for it).
 */
import { Http, createHttp, segment } from '../client.js';
import { Browser } from '../browser.js';
import { builtin, saveKey } from '../cli-config.js';
import { OyaError } from '../errors.js';
import { DESKTOP_DOWNLOAD_URL, DESKTOP_PROVIDER, DESKTOP_TIMEOUT_MS, READY_POLL_MS, Status } from '../constants.js';
import type { BrowserInfo, DesktopOptions, Signup, SignupOptions } from '../types/index.js';
import type { ConnectedBrowser, HttpRef } from './shapes.js';

/** Stands in for a key on the two signup calls, which take none. */
const NO_KEY = 'signup';

/** The puzzle `GET /api/auth/agent/challenge` sets. */
interface Challenge {
  /** The string to extend. */
  challenge: string;
  /** How many leading zero hex digits the hash needs. */
  difficulty: number;
}

/** The part of `node:crypto` the puzzle needs. */
type Crypto = {
  /** A hash to feed and read. */
  createHash(name: string): { update(s: string): { digest(enc: string): string } };
};

/** Whether sha256(challenge + nonce) starts with the zeros asked for. */
const solves = (crypto: Crypto, challenge: string, nonce: number, zeros: string) =>
  crypto
    .createHash('sha256')
    .update(challenge + nonce)
    .digest('hex')
    .startsWith(zeros);

/** A nonce that makes sha256(challenge + nonce) start with `difficulty` zeros. About a million tries. */
function solve({ challenge, difficulty }: Challenge): string {
  const crypto = builtin<Crypto>('node:crypto');
  if (!crypto) throw new Error('Oya.signup() needs Node 22.3 or newer.');
  const zeros = '0'.repeat(difficulty);
  let nonce = 0;
  while (!solves(crypto, challenge, nonce, zeros)) nonce++;
  return String(nonce);
}

/** What `POST /api/auth/agent/signup` answers. */
interface SignupAnswer {
  /** The key. */
  api_key: string;
  /** The link that claims it. */
  claim_url: string;
  /** Whether cloud browsers work yet. */
  cloud_browsers: boolean;
}

/** Signs an agent up for its own key, on behalf of the person whose email it gives. */
export async function signup(options: SignupOptions): Promise<Signup> {
  const http = createHttp({ baseUrl: options.baseUrl, fetch: options.fetch, apiKey: NO_KEY });
  const puzzle = await http.request<Challenge>('GET', '/api/auth/agent/challenge');
  const body = { email: options.email, challenge: puzzle.challenge, nonce: solve(puzzle) };
  const answer = await http.request<SignupAnswer>('POST', '/api/auth/agent/signup', body);
  const saved = options.save !== false && saveKey(answer.api_key, http.baseUrl);
  return { apiKey: answer.api_key, claimUrl: answer.claim_url, cloudBrowsers: answer.cloud_browsers, saved };
}

/** The desktop app on this key, when one is connected and alive. */
/** The live desktop app, and with `persona` (an id or name) only one signed in as that persona. */
const desktopOf = (all: BrowserInfo[], persona?: string) =>
  all.find(
    (b) =>
      b.provider === DESKTOP_PROVIDER &&
      b.health !== 'dead' &&
      (!persona || b.persona === persona || b.personaName === persona),
  );

/** The oya:// link that pairs the desktop app with this key, through a single-use code. */
async function pairingLink(http: Http, persona?: string): Promise<string> {
  const { code } = await http.request<{ /** The single-use code. */ code: string }>('POST', '/api/pairing', {
    persona,
  });
  const server = `${http.baseUrl.replace(/^http/, 'ws')}/ws`;
  return `oya://connect?code=${encodeURIComponent(code)}&server=${encodeURIComponent(server)}`;
}

/** How each OS opens a link in the app registered for it. rundll32 on Windows: `start` would read & as a command break. */
const OPENERS: Record<string, string[]> = {
  darwin: ['open'],
  linux: ['xdg-open'],
  win32: ['rundll32', 'url.dll,FileProtocolHandler'],
};

/** A started program, as much of it as opening a link needs. */
type Child = {
  /** Listens for its failure to start. */
  on(event: string, listener: () => void): void;
  /** Lets this process exit without waiting for it. */
  unref(): void;
};

/** The part of `node:child_process` opening a link needs. */
type ChildProcess = {
  /** Starts a program. */
  spawn(cmd: string, args: string[], opts: object): Child;
};

/** The global scope, which has `process` in Node. */
interface WithProcess {
  /** Node's process. */
  process?: {
    /** darwin, linux, win32… */
    platform: string;
  };
}

/** Opens the link on this machine, which hands it to the desktop app. Best effort: a failure shows in the timeout. */
function openLink(link: string): void {
  const platform = (globalThis as WithProcess).process?.platform ?? '';
  const cp = builtin<ChildProcess>('node:child_process');
  if (!cp || !Object.hasOwn(OPENERS, platform)) return;
  const [cmd, ...args] = OPENERS[platform];
  const child = cp.spawn(cmd, [...args, link], { detached: true, stdio: 'ignore' });
  child.on('error', () => {});
  child.unref();
}

/** Polls until the desktop app shows up on this key as `persona`; throws, with what to do, when it never does. */
async function waitForDesktop(list: () => Promise<BrowserInfo[]>, link: string, timeoutMs: number, persona?: string) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const found = desktopOf(await list(), persona);
    if (found) return found;
    await new Promise((r) => setTimeout(r, READY_POLL_MS));
  }
  const how = `Install it from ${DESKTOP_DOWNLOAD_URL} if needed, then open ${link} and click Connect.`;
  throw new OyaError(`The Oya desktop app did not connect. ${how}`, Status.GATEWAY_TIMEOUT, { link });
}

/** A handle on the person's desktop app. Marked reused: `stop()` must not disconnect the person's own browser. */
async function handleOn(http: Http, id: string): Promise<Browser> {
  const found = await http.request<ConnectedBrowser>('GET', `/api/browsers/${segment(id)}`);
  const started = { id, provider: DESKTOP_PROVIDER, persona: found.persona || 'default', cdpUrl: found.cdpUrl };
  return new Browser(http, { ...started, status: 'ready', reused: true }, false);
}

/** Builds `oya.desktop`; `list` is `oya.browser.list`. */
export const desktopApi = (http: HttpRef, list: () => Promise<BrowserInfo[]>) => ({
  /**
   * The person's own desktop browser, signed in to their sites. When it is not
   * connected yet this pairs it: it opens a link, the person clicks Connect in
   * the app (with "Import my logins" ticked, their Chrome logins come along),
   * and this waits until it is up. With `persona`, a running app signed in as
   * another persona is switched the same way: the link reconnects it as this one.
   */
  connect: async (options: DesktopOptions = {}): Promise<Browser> => {
    const running = desktopOf(await list(), options.persona);
    if (running) return handleOn(http(), running.id);
    const link = await pairingLink(http(), options.persona);
    if (options.open !== false) openLink(link);
    const found = await waitForDesktop(list, link, options.timeoutMs ?? DESKTOP_TIMEOUT_MS, options.persona);
    return handleOn(http(), found.id);
  },
});
