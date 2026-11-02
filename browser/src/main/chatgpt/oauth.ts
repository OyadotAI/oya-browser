/**
 * Sign in with ChatGPT's OAuth steps as plain functions: PKCE, the authorize
 * address (a first sign-in registers this app as an agent host), the token
 * requests, and reading the answer into a credential that may spend the plan.
 */
import { createHash, randomBytes } from 'node:crypto';
import * as C from './constants.ts';

/** What is kept for a ChatGPT sign-in; the same shape the server keeps on a self-hosted install. */
export interface Credential {
  /** The client id ChatGPT issued this host for the account; reused on every later sign-in. */
  client_id: string;
  /** The latest refresh token; each refresh replaces it. */
  refresh_token: string;
  /** The current access token. */
  access_token?: string;
  /** When the access token expires, ms since the epoch. */
  expires_at?: number;
  /** The ChatGPT account's email. */
  email?: string;
}

/** One sign-in in progress. */
export interface Login {
  /** The PKCE verifier. */
  verifier: string;
  /** The nonce the ID token must carry. */
  nonce: string;
  /** The state the callback must return. */
  state: string;
  /** The client the sign-in opens with. */
  clientId: string;
  /** Where the browser comes back to. */
  redirectUri: string;
}

/** What a token request may be sent with: fetch, or a stand-in in tests. */
export type Fetch = typeof fetch;

/** A fresh random value, base64url. */
export const random = () => randomBytes(C.SECRET_BYTES).toString('base64url');

/** The S256 PKCE challenge for a verifier. */
export const challengeOf = (verifier: string) => createHash('sha256').update(verifier).digest('base64url');

/** A new sign-in returning to `redirectUri`, under the client issued before or the registration client. */
export const newLogin = (redirectUri: string, savedClient?: string): Login => ({
  ...{ verifier: random(), nonce: random(), state: random() },
  ...{ clientId: savedClient || C.DYNAMIC_CLIENT, redirectUri },
});

/** The address that opens the sign-in; a first one also registers this app and names it. */
export function authorizeUrl(login: Login, hostId: string): string {
  const first: Record<string, string> = login.clientId === C.DYNAMIC_CLIENT ? { agent_name_hint: C.AGENT_NAME } : {};
  const params = new URLSearchParams({
    ...{ response_type: 'code', client_id: login.clientId, redirect_uri: login.redirectUri },
    ...{ scope: C.SCOPE, resource: C.RESOURCE, state: login.state, nonce: login.nonce, ext_agent_host_id: hostId },
    ...{ code_challenge: challengeOf(login.verifier), code_challenge_method: 'S256', ...first },
  });
  return `${C.AUTHORIZE_URL}?${params}`;
}

/** A token request OpenAI refused. */
export interface Refusal extends Error {
  /** OpenAI's OAuth error code (invalid_grant, server_error…), when it sent one. */
  code?: string;
  /** The token endpoint's HTTP status. */
  status?: number;
}

/** The error a refused token request throws, logged with OpenAI's code and description. */
function tokenRefusal(status: number, body: Record<string, string | undefined>): Refusal {
  const why = [body?.error || status, body?.error_description].filter(Boolean).join(': ');
  console.warn('[chatgpt] token request refused:', why);
  return Object.assign(Error(`ChatGPT refused the sign-in (${why}). Try again.`), { code: body?.error, status });
}

/** One form POST to the token endpoint; its JSON, or a Refusal carrying OpenAI's error code and status. */
export async function tokenRequest(form: Record<string, string>, fetchImpl: Fetch = fetch) {
  const headers = { 'Content-Type': 'application/x-www-form-urlencoded', Accept: 'application/json' };
  const init = { method: 'POST', redirect: 'error' as const, signal: AbortSignal.timeout(C.TOKEN_TIMEOUT_MS) };
  const res = await fetchImpl(C.TOKEN_URL, { ...init, headers, body: new URLSearchParams(form).toString() });
  const body = await res.json().catch(() => ({}));
  if (res.ok) return body;
  throw tokenRefusal(res.status, body);
}

/** An ID token's claims, read without verifying it: it came straight from the token endpoint over TLS. */
function claimsOf(idToken: unknown) {
  try {
    return JSON.parse(Buffer.from(String(idToken).split('.')[1], 'base64url').toString());
  } catch {
    return {};
  }
}

/** What the token endpoint answers, as far as a credential needs it. */
export interface TokenAnswer {
  /** The hour-long token model calls carry. */
  access_token?: string;
  /** The token that gets the next one. */
  refresh_token?: string;
  /** Seconds until the access token expires. */
  expires_in?: number;
  /** The scopes granted, space-separated. */
  scope?: string;
  /** Who signed in, and the nonce the sign-in sent. */
  id_token?: string;
}

/** Refuses an answer for another sign-in, or one that may not spend the plan. */
function checkGrant(tokens: TokenAnswer, signed: unknown, nonce?: string): void {
  if (nonce && signed !== nonce) throw Error('ChatGPT sign-in did not match. Try again.');
  if (
    !String(tokens.scope || '')
      .split(' ')
      .includes(C.PLAN_SCOPE)
  )
    throw Error(C.NO_PLAN_SCOPE);
}

/** The credential a token answer makes, once it may spend the plan and, at sign-in, carries the right nonce. */
export function credentialFrom(tokens: TokenAnswer, clientId: string, nonce?: string): Credential {
  const { nonce: signed, email } = claimsOf(tokens.id_token);
  checkGrant(tokens, signed, nonce);
  const expires_at = Date.now() + Number(tokens.expires_in || 0) * C.MS_PER_SECOND;
  const { refresh_token = '', access_token } = tokens;
  return { client_id: clientId, refresh_token, access_token, expires_at, email };
}

/** Statuses below 500 that are OpenAI's trouble, not a refusal of the account. */
const TRANSIENT = [C.HTTP_TIMEOUT, C.HTTP_TOO_MANY];

/** Whether OpenAI refused the account itself: an OAuth code on a 4xx that is not a timeout or rate limit. */
const refusedAccount = ({ code, status = 0 }: Refusal) =>
  typeof code === 'string' && status < C.HTTP_SERVER_ERROR && !TRANSIENT.includes(status);

/**
 * A failed code exchange in words a person can act on, never OpenAI's code: a
 * refusal of the account says the plan Oya needs; anything else (OpenAI busy,
 * rate-limited, unreachable) says to try again, since telling a paying person to
 * upgrade over OpenAI's hiccup would be wrong. The cause stays in the log.
 */
function refusedSignIn(error: Refusal): never {
  if (refusedAccount(error)) throw Error(C.SIGN_IN_REFUSED);
  console.warn('[chatgpt] sign-in exchange failed:', error.message);
  throw Error(C.SIGN_IN_FAILED);
}

/** Exchanges a sign-in's code under the client ChatGPT issued; a refusal says the plan Oya needs. Sign-in only: a refresh keeps OpenAI's code. */
export function exchangeCode(login: Login, clientId: string, code: string, fetchImpl?: Fetch) {
  const form = { grant_type: 'authorization_code', client_id: clientId, code, code_verifier: login.verifier };
  return tokenRequest({ ...form, redirect_uri: login.redirectUri, resource: C.RESOURCE }, fetchImpl).catch(
    refusedSignIn,
  );
}

/** A refreshed credential; an answer that leaves out the refresh token or scope keeps the old ones. */
export async function refreshed(credential: Credential, fetchImpl?: Fetch): Promise<Credential> {
  const { client_id, refresh_token, email } = credential;
  const form = { grant_type: 'refresh_token', client_id, refresh_token, resource: C.RESOURCE };
  const tokens = await tokenRequest(form, fetchImpl);
  return { ...credentialFrom({ refresh_token, scope: C.PLAN_SCOPE, ...tokens }, client_id), email };
}
