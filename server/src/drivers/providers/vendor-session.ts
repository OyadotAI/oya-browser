/**
 * One hosted vendor session: creating it, reading its CDP URL, releasing it,
 * and the sealed record that lets another process release it later.
 */
import { sealText } from '../../platform/secrets.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { firstPath, headersFor } from './catalog.ts';
import { PROVIDER_CREATE_TIMEOUT_MS, PROVIDER_RELEASE_TIMEOUT_MS } from '../constants.ts';

/** Throws the error the API answers with. */
export function fail(msg, status: number = Status.BAD_REQUEST): never {
  throw new HttpError(status, msg);
}

/** A parsed URL, or undefined when it does not parse. */
export function parseUrl(raw) {
  try {
    return new URL(raw);
  } catch {
    return undefined;
  }
}

/** Whether a URL is a WebSocket URL, as a CDP endpoint must be. */
export const isWebSocket = (url) => ['ws:', 'wss:'].includes(url.protocol);

/** A vendor answering 404 or 410 to a release means the session is already gone. */
export const isGone = (status) => status === Status.NOT_FOUND || status === Status.GONE;

/** Asks the vendor for a session and returns its response body (empty when it is not JSON). */
export async function createSession(provider, cfg, key, env) {
  const res = await postCreate(cfg, key, env).catch(() => fail(createUnreachable(provider), Status.BAD_GATEWAY));
  const payload = parseJson(await res.text());
  // Vendor responses can echo connection URLs or credentials. Keep them out
  // of public errors and audit records.
  if (!res.ok) fail(createRefused(provider, res.status), Status.BAD_GATEWAY);
  return payload;
}

/** The error for a create request that never got an answer. */
function createUnreachable(provider) {
  return `${provider} session create request failed. Check the provider configuration and connection.`;
}

/** The error for a create the vendor refused; only the status, never the body. */
function createRefused(provider, status) {
  return `${provider} session create failed (${status}). Check its API key, quota, and account status.`;
}

/** The create request itself. */
async function postCreate(cfg, key, env) {
  return fetch(cfg.createUrl, {
    method: 'POST',
    headers: headersFor(cfg, key),
    body: JSON.stringify(typeof cfg.body === 'function' ? cfg.body(env) : cfg.body || {}),
    signal: AbortSignal.timeout(PROVIDER_CREATE_TIMEOUT_MS),
    redirect: 'error',
  });
}

/** A response body as JSON, or {} when it is not. */
function parseJson(text) {
  try {
    return JSON.parse(text);
  } catch {
    return {};
  }
}

/** The CDP URL from the vendor's response, with the key added when the vendor wants it in the query. */
export function cdpUrl(provider, cfg, key, payload) {
  const url = parseUrl(firstPath(payload, cfg.wsPath || []));
  if (!url || !isWebSocket(url)) {
    fail(`${provider} responded without a valid CDP URL. Check the provider configuration.`, Status.BAD_GATEWAY);
  }
  if (cfg.wsQueryKey) url.searchParams.set(cfg.wsQueryKey, key);
  return url.href;
}

/** Where the vendor releases this session; config may give a function or a URL template with {id}. */
function deleteEndpoint(cfg, sessionId) {
  const id = encodeURIComponent(sessionId);
  return typeof cfg.deleteUrl === 'function' ? cfg.deleteUrl(id) : cfg.deleteUrl.replace('{id}', id);
}

/** Method, headers and body of the vendor's release call. */
function deleteOptions(cfg, key) {
  return {
    method: cfg.deleteMethod || 'DELETE',
    headers: headersFor(cfg, key),
    ...(cfg.deleteBody ? { body: JSON.stringify(cfg.deleteBody) } : {}),
  };
}

/** Releases the session, at most once; a failed release may be tried again. */
export function releaser(provider, cfg, key, sessionId) {
  const current = { release: null };
  return () => {
    if (!sessionId || !cfg.deleteUrl) return Promise.resolve();
    current.release ||= releaseSession(provider, cfg, key, sessionId).catch((err) => forget(current, err));
    return current.release;
  };
}

/** A failed release is forgotten, so the next call tries again. */
function forget(current, err): never {
  current.release = null;
  throw err;
}

/** One release call; a session the vendor no longer has counts as released. */
async function releaseSession(provider, cfg, key, sessionId) {
  const endpoint = deleteEndpoint(cfg, sessionId);
  const released = await requestRelease(endpoint, cfg, key).catch(() =>
    fail(`${provider} session ${sessionId} release request failed`, Status.BAD_GATEWAY),
  );
  if (!released.ok && !isGone(released.status)) {
    fail(`${provider} session ${sessionId} release failed (${released.status})`, Status.BAD_GATEWAY);
  }
}

/** The release request itself. */
async function requestRelease(endpoint, cfg, key) {
  const options = { ...deleteOptions(cfg, key), signal: AbortSignal.timeout(PROVIDER_RELEASE_TIMEOUT_MS) };
  return fetch(endpoint, { ...options, redirect: 'error' });
}

/** The sealed release call, handed to onCreated so the control worker can release a session this process lost. */
export function cleanupRecord(cfg, key, sessionId) {
  if (!sessionId || !cfg.deleteUrl) return null;
  const call = { url: deleteEndpoint(cfg, sessionId), ...deleteOptions(cfg, key) };
  return { kind: 'vendor', sealed: sealText('provider-cleanup', call) };
}
