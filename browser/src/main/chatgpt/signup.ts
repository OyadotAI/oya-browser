/**
 * An Oya key without an Oya account: the server's agent sign-up, which asks for
 * an email and a little proof of work instead of a password. The key works at
 * once (its trial steps, this app, the person's own model) and is claimed into an
 * account later through its claim link.
 */
import { createHash } from 'node:crypto';
import { serverHttpBase, readApiAnswer } from '../connection/server-api.ts';
import * as C from './constants.ts';
import { boundedSignal } from './oauth.ts';

/** What the server's puzzle looks like. */
interface Challenge {
  /** The sealed challenge. */
  challenge: string;
  /** How many leading hex zeros the hash needs. */
  difficulty: number;
}

/** A new key and where to claim it. */
export interface SignedUp {
  /** The key. */
  apiKey: string;
  /** Where the person claims it into an account. */
  claimUrl: string;
}

/** Whether `nonce` solves the puzzle: sha256(challenge + nonce) starts with the zeros. */
const solves = (challenge: string, nonce: number, zeros: string) =>
  createHash('sha256')
    .update(challenge + nonce)
    .digest('hex')
    .startsWith(zeros);

/** The nonce that solves the puzzle. ponytail: runs in main (about a second); a worker if it janks. */
export function solve({ challenge, difficulty }: Challenge): string {
  const zeros = '0'.repeat(difficulty);
  for (let n = 0; n < C.POW_MAX_TRIES; n++) if (solves(challenge, n, zeros)) return String(n);
  throw Error('Could not finish signing up. Try again.');
}

/** POSTs `body` as JSON to the server's `route`; its answer, or the refusal (a 429 named for what it means). */
async function post(base: string, route: string, body: object, fetchImpl: typeof fetch, signal?: AbortSignal) {
  const init = { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) };
  return readApiAnswer(await fetchImpl(`${base}/api/${route}`, { ...init, signal })).catch((err) => {
    throw err?.status === C.HTTP_TOO_MANY ? Error(C.TOO_MANY_SIGNUPS) : err;
  });
}

/** What the sign-up answers. */
interface SignupAnswer {
  /** The new key. */
  api_key: string;
  /** Where to claim it. */
  claim_url: string;
}

/** Signs up for a key on the server at `serverUrl` (its ws address), for `email`. */
export async function signUp(
  serverUrl: string,
  email: string,
  fetchImpl: typeof fetch = fetch,
  signal?: AbortSignal,
): Promise<SignedUp> {
  const base = serverHttpBase(serverUrl);
  const bounded = boundedSignal(signal);
  return signUpAt(base, email, fetchImpl, bounded);
}

/** Challenge and signup share one cancellation/deadline and destination. */
async function signUpAt(base: string, email: string, fetchImpl: typeof fetch, bounded: AbortSignal): Promise<SignedUp> {
  const puzzle = (await readApiAnswer(
    await fetchImpl(`${base}/api/auth/agent/challenge`, { signal: bounded }),
  )) as Challenge;
  bounded.throwIfAborted();
  const body = { email, challenge: puzzle.challenge, nonce: solve(puzzle) };
  const answer = (await post(base, 'auth/agent/signup', body, fetchImpl, bounded)) as SignupAnswer;
  return { apiKey: answer.api_key, claimUrl: answer.claim_url };
}
