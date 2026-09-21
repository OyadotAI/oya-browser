/**
 * Outbound destination guard.
 *
 * The control plane dials URLs that callers supply, CDP providers, model
 * endpoints. Its network position is not the caller's, so an unchecked URL is
 * a server-side request forgery primitive: cloud metadata, a loopback admin
 * port, anything routable from the host.
 *
 * Loopback and private ranges are refused unless the host opts in with
 * OYA_ALLOW_PRIVATE_TARGETS=true. That opt-in exists because a single-operator
 * self-hosted install legitimately points at ws://127.0.0.1:9222; it should
 * never be on for a deployment where keys belong to other people.
 */

import { lookup } from 'dns/promises';
import { isIP } from 'net';
import { HttpError } from './errors.ts';
import { Status } from './http-status.ts';
import { IPV4_MULTICAST_FIRST_OCTET, IPV6, NEVER_ALLOWED_V4, PRIVATE_V4, type Ipv4Range } from './constants.ts';

const allowPrivate = () => process.env.OYA_ALLOW_PRIVATE_TARGETS === 'true';

/** An IPv4-mapped IPv6 address (::ffff:127.0.0.1), capturing the v4 part. */
const V4_MAPPED = /^::ffff:(\d+\.\d+\.\d+\.\d+)$/;

/** True when the IPv4 address falls in one of the ranges. */
function inRanges(ip, ranges: Ipv4Range[]) {
  const [a, b] = ip.split('.').map(Number);
  return ranges.some((r) => a === r.first && (r.from === undefined || (b >= r.from && b <= r.to)));
}

/**
 * Never routable on a caller's behalf, opt-in or not. Link-local carries the
 * cloud metadata service (169.254.169.254) and has no legitimate use as a
 * browser endpoint, so the self-hosting escape hatch must not cover it.
 */
export function isNeverAllowed(ip) {
  const v = isIP(ip);
  if (!v) return true;
  if (v === IPV6) return isNeverAllowedV6(ip);
  return inRanges(ip, NEVER_ALLOWED_V4) || Number(ip.split('.')[0]) >= IPV4_MULTICAST_FIRST_OCTET;
}

/** isNeverAllowed for IPv6: link-local and the unspecified address, or the mapped v4 address. */
function isNeverAllowedV6(ip) {
  const s = ip.toLowerCase();
  const mapped = s.match(V4_MAPPED);
  if (mapped) return isNeverAllowed(mapped[1]);
  return /^fe[89ab]/.test(s) || s === '::';
}

/** True for addresses that are not safely routable on behalf of a caller. */
export function isPrivateAddress(ip) {
  const v = isIP(ip);
  if (!v) return true; // unresolvable is not safe
  if (v === IPV6) return isPrivateV6(ip);
  // multicast, reserved
  return inRanges(ip, PRIVATE_V4) || Number(ip.split('.')[0]) >= IPV4_MULTICAST_FIRST_OCTET;
}

/** isPrivateAddress for IPv6: loopback, unspecified, unique local and link-local. */
function isPrivateV6(ip) {
  const s = ip.toLowerCase();
  // IPv4-mapped (::ffff:127.0.0.1) must be judged as the v4 address.
  const mapped = s.match(V4_MAPPED);
  if (mapped) return isPrivateAddress(mapped[1]);
  if (s === '::1' || s === '::') return true;
  if (/^f[cd]/.test(s)) return true; // unique local fc00::/7
  return /^fe[89ab]/.test(s); // link-local fe80::/10
}

/**
 * Validate a caller-supplied URL and resolve it, so a public hostname pointing
 * at an internal address is caught too.
 *
 * ponytail: resolves here, connects later, a name could change in between
 * (DNS rebinding). Closing that needs the socket pinned to the address checked,
 * which means a custom agent/lookup on every dial site. Re-validated at connect
 * time as well, which narrows the window to the dial itself.
 *
 * @returns {Promise<{ href: string, hostname: string, addresses: string[] }>}
 */
export async function assertSafeTarget(raw, { protocols = ['ws:', 'wss:'], label = 'URL' } = {}) {
  const fail = failWith(label);
  const url = parseTarget(raw, protocols, fail);
  const host = url.hostname.replace(/^\[|\]$/g, '');
  const addresses = await resolve(host, fail);
  refuseReserved(addresses, fail);
  if (!allowPrivate()) refusePrivate(addresses, fail);
  return { href: url.href, hostname: host, addresses };
}

/** Rejects a failed check with the reason. */
type Fail = (why: string) => never;

/** A Fail that answers 400 with the reason after the label. */
const failWith =
  (label: string): Fail =>
  (why) => {
    throw new HttpError(Status.BAD_REQUEST, `${label} ${why}`);
  };

/** The URL, checked for scheme and embedded credentials. */
function parseTarget(raw, protocols: string[], fail: Fail) {
  const url = parseUrl(raw, fail);
  if (!protocols.includes(url.protocol)) fail(`must use ${protocols.join(' or ')}`);
  if (url.username || url.password) fail('must not embed credentials');
  return url;
}

/** Parse the raw value as a URL, or fail. */
function parseUrl(raw, fail: Fail) {
  try {
    return new URL(String(raw));
  } catch {
    return fail('is not a valid URL');
  }
}

/** Every address the host resolves to; an IP literal is its own answer. */
async function resolve(host, fail: Fail) {
  const addresses = isIP(host) ? [host] : await lookupAll(host, fail);
  if (!addresses.length) fail(`hostname could not be resolved (${host})`);
  return addresses;
}

/** Resolve a hostname through DNS, or fail. */
async function lookupAll(host, fail: Fail) {
  try {
    return (await lookup(host, { all: true })).map((a) => a.address);
  } catch {
    return fail(`hostname could not be resolved (${host})`);
  }
}

/** Fail when any address is link-local or reserved. */
function refuseReserved(addresses, fail: Fail) {
  const never = addresses.filter(isNeverAllowed);
  if (!never.length) return;
  fail(
    `resolves to a link-local or reserved address (${never[0]}), which is never dialled ` +
      'on behalf of a caller. That range carries the cloud metadata service.',
  );
}

/** Fail when any address is private or loopback. */
function refusePrivate(addresses, fail: Fail) {
  const blocked = addresses.filter(isPrivateAddress);
  if (!blocked.length) return;
  fail(
    `resolves to a private or loopback address (${blocked[0]}). ` +
      'Set OYA_ALLOW_PRIVATE_TARGETS=true only on a single-operator host where every ' +
      'API key is yours.',
  );
}
