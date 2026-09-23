/**
 * Agent self-signup's guards. An AI agent registers itself with no person and
 * no captcha, so three things stand in for one: a proof of work (a hash puzzle
 * that costs one agent a second or two and a signup farm a lot), a per-address
 * daily limit, and an email naming the person the agent works for.
 *
 * A challenge is sealed with the server's secret rather than stored, so any
 * instance can check one another issued.
 */
import { createHash } from 'node:crypto';
import { openText, sealText } from '../../platform/secrets.ts';
import { HttpError } from '../../platform/errors.ts';
import { consoleUrl } from '../slack/service.ts';
import { Status } from '../../platform/http-status.ts';
import { AGENT_CHALLENGE_TTL_MS, AGENT_EMAIL_MAX_CHARS, AGENT_NONCE_MAX_CHARS, AGENT_POW_ZEROS } from './constants.ts';

/** What a challenge is sealed under, so no other sealed value passes for one. */
const SCOPE = 'agent-challenge';

/**
 * Challenges already used, until they expire.
 * ponytail: per process; with several replicas one solved challenge could sign
 * up once on each. The daily per-address limit still bounds it. Move to the
 * control store if replicas multiply.
 */
const spent = new Map<string, number>();

/** A fresh puzzle: find a nonce so that sha256(challenge + nonce) starts with `difficulty` hex zeros. */
export function issueChallenge() {
  const expires = Date.now() + AGENT_CHALLENGE_TTL_MS;
  return {
    challenge: sealText(SCOPE, { expires }),
    difficulty: AGENT_POW_ZEROS,
    expires_at: new Date(expires).toISOString(),
  };
}

/** Whether `nonce` solves `challenge`. */
export const solves = (challenge: string, nonce: string) =>
  createHash('sha256')
    .update(challenge + nonce)
    .digest('hex')
    .startsWith('0'.repeat(AGENT_POW_ZEROS));

/** When a challenge this server sealed expires, or 0 for anything else. */
function expiryOf(challenge: unknown) {
  try {
    return typeof challenge === 'string' ? Number(openText(SCOPE, challenge)?.expires) || 0 : 0;
  } catch {
    return 0;
  }
}

/** Drops spent challenges that have expired anyway. */
function forgetExpired(now: number) {
  for (const [challenge, expires] of spent) if (expires < now) spent.delete(challenge);
}

/** What is wrong with a signup's challenge and nonce, or '' when it may go ahead. */
function problemWith(challenge: unknown, nonce: unknown, now: number) {
  if (expiryOf(challenge) < now) return 'Challenge invalid or expired: GET /auth/agent/challenge for a new one';
  if (spent.has(challenge as string)) return 'Challenge already used';
  const solved =
    typeof nonce === 'string' && nonce.length <= AGENT_NONCE_MAX_CHARS && solves(challenge as string, nonce);
  return solved ? '' : `nonce does not solve the challenge (need ${AGENT_POW_ZEROS} leading hex zeros)`;
}

/** Accepts a solved, unexpired, unused challenge once; anything else is a 400 saying what to do. */
export function spendChallenge(challenge: unknown, nonce: unknown) {
  const now = Date.now();
  forgetExpired(now);
  const problem = problemWith(challenge, nonce, now);
  if (problem) throw new HttpError(Status.BAD_REQUEST, problem);
  spent.set(challenge as string, expiryOf(challenge));
}

/** A plausible email: one @, something on both sides, a dot in the domain, no spaces. */
export const validEmail = (email: unknown): email is string =>
  typeof email === 'string' && email.length <= AGENT_EMAIL_MAX_CHARS && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);

/**
 * The caller's address. Behind the ingress the socket is the proxy, so it is
 * the last X-Forwarded-For hop, the one the ingress itself appended; earlier
 * hops are whatever the client claimed and would let it pick its own bucket.
 */
export function clientAddress(req) {
  const hops = String(req.headers?.['x-forwarded-for'] || '')
    .split(',')
    .map((hop) => hop.trim())
    .filter(Boolean);
  return hops.at(-1) || req.socket?.remoteAddress || 'unknown';
}

/** Where a person claims an agent's key: the key rides in the fragment, which never reaches a server log. */
export const claimUrl = (key: string) => `${consoleUrl()}/claim#${key}`;
