/**
 * `oya.browser`: start, reattach to, list and stop browsers.
 */
import { headerSafe, segment } from '../client.js';
import { Browser } from '../browser.js';
import { READY_TIMEOUT_MS, START_TIMEOUT_MS } from '../constants.js';
import type { BrowserInfo, StartOptions, StartResult } from '../types/index.js';
import type { ConnectedBrowser, HttpRef, StopManyResult } from './shapes.js';

/** Waits until a starting browser has dialled in. */
export type WaitUntilConnected = (id: string, timeoutMs: number) => Promise<void>;

/** The body `POST /api/browsers/start` takes. */
function startBody(options: StartOptions) {
  const { profile, persona, provider, wsUrl, name, queueMs, priority, budgetUsd, governed, policy } = options;
  return { profile: profile || persona, provider, wsUrl, name, queueMs, priority, budgetUsd, governed, policy };
}

/** The caller's idempotency key, refused before the call when a header could not carry it; a fresh one otherwise. */
const idempotencyKeyOf = ({ idempotencyKey }: StartOptions) =>
  idempotencyKey
    ? headerSafe(idempotencyKey, 'idempotencyKey', 'Use letters, digits, "-" and "_".')
    : globalThis.crypto.randomUUID();

/** Starts a browser; a cloud one is waited for, then asked for its CDP URL. */
async function start(http: HttpRef, wait: WaitUntilConnected, options: StartOptions): Promise<Browser> {
  const headers = { 'Idempotency-Key': idempotencyKeyOf(options) };
  const path = '/api/browsers/start';
  const started = await http().request<StartResult>('POST', path, startBody(options), START_TIMEOUT_MS, headers);
  // Cloud browsers dial in themselves, so 'starting' means "not yet".
  if (started.status === 'starting') {
    await wait(started.id, options.readyTimeoutMs ?? READY_TIMEOUT_MS + (options.queueMs || 0));
    started.cdpUrl = (await fetchBrowser(http, started.id)).cdpUrl;
  }
  return new Browser(http(), started, options.captcha === 'auto');
}

/** One browser's record. */
const fetchBrowser = (http: HttpRef, id: string) =>
  http().request<ConnectedBrowser>('GET', `/api/browsers/${segment(id)}`);

/** A Browser for one that is already running. */
async function reattach(http: HttpRef, id: string): Promise<Browser> {
  const found = await fetchBrowser(http, id);
  return new Browser(http(), asStarted(found), false);
}

/** A running browser's record, in the shape `start()` answers with. */
function asStarted(found: ConnectedBrowser): StartResult {
  const provider = found.provider || 'cdp';
  return { id: found.id, provider, persona: found.persona || 'default', status: 'ready', cdpUrl: found.cdpUrl };
}

/** Stops some browsers, or every one on this key. */
const stopBrowsers = (http: HttpRef, ids: string[] | 'all'): Promise<StopManyResult> =>
  http().request('POST', '/api/browsers/stop', ids === 'all' ? { all: true } : { ids }, START_TIMEOUT_MS);

/** Builds `oya.browser`. */
export const browserApi = (http: HttpRef, wait: WaitUntilConnected) => ({
  /** Start a browser and wait until it can take commands. */
  start: async (options: StartOptions = {}): Promise<Browser> => start(http, wait, options),
  /** Reattach to a browser that is already running. */
  get: async (id: string): Promise<Browser> => reattach(http, id),
  /** Every browser on this key. */
  list: async (): Promise<BrowserInfo[]> => http().request<BrowserInfo[]>('GET', '/api/browsers'),
  /** Stop some (`ids`) or every browser on this key. Each reports separately. */
  stop: async (ids: string[] | 'all'): Promise<StopManyResult> => stopBrowsers(http, ids),
  /** Stop every browser on this key; returns how many stopped. */
  stopAll: async (): Promise<number> => (await stopBrowsers(http, 'all')).stopped,
});
