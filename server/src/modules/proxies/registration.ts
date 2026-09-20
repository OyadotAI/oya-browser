/**
 * Registering a proxy: the URL is checked (scheme, SOCKS auth, SSRF), then its
 * credentials are split out and sealed, never kept in the URL.
 */
import { sealText, haveSecret } from '../../platform/secrets.ts';
import { assertSafeTarget } from '../../platform/net-guard.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { Proxy, newProxyId } from './proxy.ts';
import { proxies, scopeFor } from './store.ts';
import { PROXY_PROTOCOLS } from './constants.ts';

/** Add a proxy for an owner (null = shared). Credentials are split out of the URL and sealed; unsafe or unusable URLs are refused. */
export async function register({ owner = null, label, url, geo, kind, maxPersonas }) {
  requireSecret();
  const parsed = parseProxyUrl(url);
  await assertSafeProxy(parsed);
  const id = newProxyId();
  const proxy = new Proxy({ id, owner, label, geo, kind, maxPersonas, sealed: sealCredentials(id, parsed) });
  proxies.set(id, proxy);
  return proxy;
}

/** Credentials cannot be stored without the sealing secret; refused with a 409. */
function requireSecret() {
  if (!haveSecret()) {
    throw new HttpError(Status.CONFLICT, 'OYA_PROFILE_SECRET is required before proxy credentials can be stored');
  }
}

/** The URL, refused with a 400 when it is not a proxy Chromium can use. */
function parseProxyUrl(url) {
  const parsed = toUrl(url);
  if (!PROXY_PROTOCOLS.includes(parsed.protocol)) {
    throw new HttpError(Status.BAD_REQUEST, 'proxy url must be http, https or socks5');
  }
  refuseSocksAuth(parsed);
  return parsed;
}

/**
 * Chromium does not implement SOCKS5 username/password auth — it drops the
 * credentials silently, and most residential vendors sell exactly that form.
 * Better to refuse than to hand back an exit that quietly does not apply.
 */
function refuseSocksAuth(parsed) {
  if (parsed.protocol.startsWith('socks') && (parsed.username || parsed.password)) {
    throw new HttpError(
      Status.BAD_REQUEST,
      'Chromium cannot authenticate SOCKS5 proxies. Use an http proxy, or terminate auth locally.',
    );
  }
}

/** Parses the URL, or refuses it with a 400. */
function toUrl(url) {
  try {
    return new URL(url);
  } catch {
    throw new HttpError(Status.BAD_REQUEST, 'proxy url must be a valid URL');
  }
}

/**
 * The server dials this address during health checks, using its own network
 * position rather than the caller's — the same SSRF primitive as a
 * caller-supplied CDP endpoint. Private and loopback are refused unless the
 * host opts in, and link-local (cloud metadata) never.
 * Validate the address only — credentials are the normal form for a proxy,
 * and they are stripped and encrypted below rather than kept in the URL.
 */
async function assertSafeProxy(parsed) {
  await assertSafeTarget(`${parsed.protocol}//${parsed.host}`, {
    protocols: PROXY_PROTOCOLS,
    label: 'proxy url',
  });
}

/**
 * The URL's address and credentials, sealed under the proxy's scope. URL keeps
 * userinfo percent-encoded, so a password with `:` or `@` in it is decoded here,
 * the way residential.ts does, or the proxy would be sent `pa%3Ass` for `pa:ss`.
 */
function sealCredentials(id, parsed) {
  return sealText(scopeFor(id), {
    url: `${parsed.protocol}//${parsed.host}`,
    username: decodeURIComponent(parsed.username) || null,
    password: decodeURIComponent(parsed.password) || null,
  });
}
