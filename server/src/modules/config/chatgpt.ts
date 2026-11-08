/**
 * Sign in with ChatGPT, so a self-hosted Oya runs its agent on the person's
 * ChatGPT Plus or Pro plan instead of an API key. This is OpenAI's plan usage for
 * open-source and locally hosted apps: the first sign-in registers this server as
 * an agent host (no partner client id needed) and returns to a loopback address,
 * so the browser signing in must run on the same machine as the server. A hosted
 * deployment may not use it: remotely hosted apps need OpenAI's approval.
 *
 * The credential (issued client id, rotating refresh token, the current hour-long
 * access token) is kept sealed as the key's openai_api_key. resolveFresh() hands
 * callers a live access token, refreshing it one request at a time per key, since
 * every refresh rotates the refresh token and two at once would race.
 */
import { createHash, randomBytes, randomUUID } from 'node:crypto';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { MS_PER_SECOND } from '../../platform/constants.ts';
import { hosted } from '../billing/config.ts';
import { control, hash, keyOfProject, projectId } from '../control/service.ts';
import { LLM_DEFAULTS } from './llm-catalog.ts';
import { resolve, resolveSaved, set, type LlmConfig } from './service.ts';
import { checkRequestLlm, requestLlm } from './request-llm.ts';
import * as C from './constants.ts';
import { credentialOf, desktopHeld, type Credential } from './chatgpt-credential.ts';
import { desktopRelay } from '../browsers/connection/model-relay.ts';

export { credentialOf };

/** A sign-in waiting for its callback. */
interface Login {
  /** The project that started it. */
  project: string;
  /** The PKCE verifier. */
  verifier: string;
  /** The nonce the ID token must carry. */
  nonce: string;
  /** The client the sign-in was opened with. */
  clientId: string;
  /** When it lapses. */
  expiresAt: number;
}

/** A fresh random value, base64url. */
const random = () => randomBytes(C.CHATGPT_SECRET_BYTES).toString('base64url');

/** The S256 PKCE challenge for a verifier. */
const challengeOf = (verifier: string) => createHash('sha256').update(verifier).digest('base64url');

/** The loopback address sign-in returns to: this server, as the browser on the same machine reaches it. */
export const redirectUri = () => `http://127.0.0.1:${publicPort()}${C.CHATGPT_CALLBACK_PATH}`;

/** The port the browser reaches this server on: in Docker that is the host's published one, not the container's. */
const publicPort = () => process.env.OYA_PUBLIC_PORT || process.env.PORT || C.DEFAULT_PORT;

/** This server's agent host id: made once, kept, and never carrying anything about its owner. */
async function hostId(): Promise<string> {
  return control().store.transact(async (tx) => {
    const row = await tx.get('meta', 'chatgpt_host');
    if (row?.hostId) return row.hostId;
    const hostId = `urn:uuid:${randomUUID()}`;
    tx.put('meta', 'chatgpt_host', { id: 'chatgpt_host', hostId });
    return hostId;
  });
}

/**
 * The client ChatGPT issued this project, kept apart from the credential so a
 * sign-in whose code exchange was refused still leaves it: OpenAI wants the retry
 * under that client, and registering afresh is refused again.
 */
async function savedClient(project: string): Promise<string | undefined> {
  return control().store.transact(async (tx) => (await tx.get('meta', `chatgpt_client:${project}`))?.clientId);
}

/** Keeps the client ChatGPT issued this project, before its code is exchanged. */
async function keepClient(project: string, clientId: string) {
  const id = `chatgpt_client:${project}`;
  // Read before writing: rows are versioned, and a write over a row this transaction never read conflicts.
  await control().store.transact(async (tx) => {
    if ((await tx.get('meta', id))?.clientId !== clientId) tx.put('meta', id, { id, clientId });
  });
}

/** The authorize URL's parameters; a first sign-in registers through the dynamic client and names the app. */
function authorizeParams(login: Login, state: string, host: string) {
  const first = login.clientId === C.CHATGPT_DYNAMIC_CLIENT ? { agent_name_hint: C.CHATGPT_AGENT_NAME } : {};
  return new URLSearchParams({
    ...{ response_type: 'code', client_id: login.clientId, redirect_uri: redirectUri(), state, nonce: login.nonce },
    ...{ scope: C.CHATGPT_SCOPE, resource: C.CHATGPT_RESOURCE, ext_agent_host_id: host, ...first },
    ...{ code_challenge: challengeOf(login.verifier), code_challenge_method: 'S256' },
  });
}

/** A server-side sign-in is for a self-hosted server; it also makes sure the key's project row exists, which the callback finds the key through. */
async function admitLogin(apiKey: string) {
  if (hosted())
    throw new HttpError(Status.FORBIDDEN, 'On Oya Cloud, sign in with ChatGPT from Oya Browser on your computer');
  await control().project(apiKey);
}

/** Starts a sign-in for this key; the URL to open in a browser on this machine. */
export async function startLogin(apiKey: string): Promise<string> {
  await admitLogin(apiKey);
  const issued = credentialOf(resolve(apiKey).openaiKey)?.client_id || (await savedClient(projectId(apiKey)));
  const clientId = issued || C.CHATGPT_DYNAMIC_CLIENT;
  const login: Login = { project: projectId(apiKey), verifier: random(), nonce: random(), clientId, expiresAt: 0 };
  login.expiresAt = Date.now() + C.CHATGPT_LOGIN_TTL_MS;
  const state = random();
  await control().store.transact(async (tx) => tx.put('chatgpt_login', hash(state), { id: hash(state), ...login }));
  return `${C.CHATGPT_AUTHORIZE_URL}?${authorizeParams(login, state, await hostId())}`;
}

/** Takes a sign-in out of the store, once; null when unknown or lapsed. */
async function takeLogin(state: string): Promise<Login | null> {
  if (!state) return null;
  const login = await control().store.transact(async (tx) => {
    const found = await tx.get('chatgpt_login', hash(state));
    if (found) await tx.delete('chatgpt_login', hash(state));
    return found;
  });
  return login && login.expiresAt > Date.now() ? login : null;
}

/** One form POST to the token endpoint; its JSON, or the refusal (logged, never echoed). */
async function tokenRequest(form: Record<string, string>, refused = EXPIRED) {
  const init = { method: 'POST', redirect: 'error' as const, signal: AbortSignal.timeout(C.CHATGPT_TOKEN_TIMEOUT_MS) };
  const headers = { 'Content-Type': 'application/x-www-form-urlencoded', Accept: 'application/json' };
  const res = await fetch(C.CHATGPT_TOKEN_URL, { ...init, headers, body: new URLSearchParams(form).toString() });
  const body = await res.json().catch(() => ({}));
  if (res.ok) return body;
  logRefusal(res.status, form.grant_type, body);
  throw new HttpError(Status.UNPROCESSABLE, refused);
}

/** Logs why the token endpoint refused: its error code and the start of its description, never the form. */
function logRefusal(status: number, grant: string, body) {
  // OpenAI's whole refusal, as it is theirs and holds nothing of ours; cut short all the same.
  console.error(
    `[chatgpt] token endpoint ${status} for ${grant}: ${JSON.stringify(body).slice(0, C.CHATGPT_LOG_CHARS)}`,
  );
}

/** What a refused refresh tells the person. */
const EXPIRED = 'ChatGPT sign-in has expired or was revoked; sign in with ChatGPT again';
/** What a refused sign-in tells the person. */
const NOT_FINISHED = 'ChatGPT did not finish the sign-in. Start it again from Oya.';

/** An ID token's claims, read without verifying its signature: it came straight from the token endpoint over TLS. */
function claimsOf(idToken: string) {
  try {
    return JSON.parse(Buffer.from(String(idToken).split('.')[1], 'base64url').toString());
  } catch {
    return {};
  }
}

/** Whether a token response grants spending the plan, not just signing in. */
const planGranted = (tokens) =>
  String(tokens.scope || '')
    .split(' ')
    .includes(C.CHATGPT_PLAN_SCOPE);

/** The credential a token response makes, once it carries the plan permission and, at sign-in, the right nonce. */
function credentialFrom(tokens, clientId: string, nonce?: string): Credential {
  const { nonce: signed, email } = claimsOf(tokens.id_token);
  if (nonce && signed !== nonce) throw new HttpError(Status.BAD_REQUEST, 'ChatGPT sign-in did not match');
  if (!planGranted(tokens)) throw new HttpError(Status.FORBIDDEN, 'ChatGPT did not allow using your plan in Oya');
  const expires_at = Date.now() + Number(tokens.expires_in || 0) * MS_PER_SECOND;
  const { refresh_token, access_token } = tokens;
  return { client_id: clientId, refresh_token, access_token, expires_at, email };
}

/** The latest credential per key, access token included even when storage could not hold it. */
const live = new Map<string, Credential>();

/** Saves the credential on the key, without the access token when the whole would not fit. */
async function save(apiKey: string, credential: Credential, extra = {}) {
  live.set(apiKey, credential);
  const full = JSON.stringify(credential);
  const { access_token: _, expires_at: __, ...lean } = credential;
  const text = full.length <= C.CONFIG_VALUE_MAX_CHARS ? full : JSON.stringify(lean);
  await set(apiKey, { ...extra, openai_api_key: text });
}

/** Exchanges a sign-in's code (with its PKCE verifier) for tokens, under the client ChatGPT issued. */
function exchangeCode(login: Login, clientId: string, code: string) {
  const shape = (v: string) => `${v.length} chars, ${/^[\w.-]*$/.test(v) ? 'url-safe' : 'other chars'}`;
  console.log(`[chatgpt] exchanging: code ${shape(code)}; verifier ${shape(login.verifier)}; return ${redirectUri()}`);
  const form = { grant_type: 'authorization_code', client_id: clientId, code, code_verifier: login.verifier };
  return tokenRequest({ ...form, redirect_uri: redirectUri(), resource: C.CHATGPT_RESOURCE }, NOT_FINISHED);
}

/** The sign-in a callback answers, once; refused when it lapsed or the person said no. */
async function answeredLogin(query: Record<string, any>): Promise<Login> {
  const login = await takeLogin(String(query.state || ''));
  if (!login) throw new HttpError(Status.GONE, 'This sign-in expired. Start it again from Oya.');
  return login;
}

/** Runs the rest of a sign-in for `apiKey`; a failure carries the key, so its desktops can be told to stop waiting. */
async function forKey<T>(apiKey: string, work: () => Promise<T>): Promise<T> {
  try {
    return await work();
  } catch (err) {
    throw Object.assign(err, { apiKey });
  }
}

/**
 * The client id to exchange the code under: the one the callback issued, else (a
 * re-sign-in, which may leave it out) the one the sign-in started with. The dynamic
 * registration client is never exchanged under: a first sign-in that came back
 * without an issued id did not finish registering. A re-sign-in answered for
 * another client is refused, as OpenAI asks, rather than replacing this one.
 */
function issuedClient(query: Record<string, any>, login: Login): string {
  const issued = String(query.client_id || '');
  console.log(
    `[chatgpt] sign-in returned: ${Object.keys(query).sort().join(', ')} (client ${issued.slice(0, C.CHATGPT_ID_LOG_CHARS)}…)`,
  );
  const clientId = String(query.client_id || login.clientId);
  if (clientId === C.CHATGPT_DYNAMIC_CLIENT) throw new HttpError(Status.BAD_GATEWAY, NOT_FINISHED);
  return sameClient(clientId, login);
}

/** `clientId`, unless a re-sign-in came back for a client other than the one it opened with. */
function sameClient(clientId: string, login: Login): string {
  if (login.clientId !== C.CHATGPT_DYNAMIC_CLIENT && clientId !== login.clientId)
    throw new HttpError(Status.BAD_REQUEST, 'ChatGPT sign-in did not match');
  return clientId;
}

/** The rest of a sign-in once its login and key are known: refuse a cancel, exchange the code, save. */
async function completeLogin(query: Record<string, any>, login: Login, apiKey: string) {
  if (query.error) throw new HttpError(Status.FORBIDDEN, 'ChatGPT sign-in was cancelled or not allowed');
  const clientId = issuedClient(query, login);
  await keepClient(login.project, clientId);
  const tokens = await exchangeCode(login, clientId, String(query.code || ''));
  const credential = credentialFrom(tokens, clientId, login.nonce);
  await save(apiKey, credential, { llm_provider: 'chatgpt', openai_base_url: null, chat_model: null });
  return { apiKey, email: credential.email || '' };
}

/** Finishes a sign-in from its callback: the key now runs on the ChatGPT plan. Answers the key and the account's email. */
export async function finishLogin(query: Record<string, any>) {
  const login = await answeredLogin(query);
  const apiKey = await keyOfProject(login.project);
  return forKey(apiKey, () => completeLogin(query, login, apiKey));
}

/** Refreshes a credential, saves the rotated one, and answers it. A refresh keeps the grant, scope included. */
async function refresh(apiKey: string, credential: Credential): Promise<Credential> {
  const { client_id, refresh_token, email } = credential;
  const form = { grant_type: 'refresh_token', client_id, refresh_token };
  const tokens = await tokenRequest({ ...form, resource: C.CHATGPT_RESOURCE });
  const kept = { refresh_token, scope: C.CHATGPT_PLAN_SCOPE, ...tokens };
  const next = { ...credentialFrom(kept, client_id), email };
  await save(apiKey, next);
  return next;
}

/** Refreshes in flight, per key: a second caller waits for the first instead of racing the rotating token. */
const inFlight = new Map<string, Promise<Credential>>();

/** The credential with an access token good for a while yet, refreshed (once at a time) when it is not. */
function fresh(apiKey: string, credential: Credential): Promise<Credential> {
  if (credential.access_token && (credential.expires_at || 0) - Date.now() > C.CHATGPT_REFRESH_EARLY_MS)
    return Promise.resolve(credential);
  if (!inFlight.has(apiKey))
    inFlight.set(
      apiKey,
      refresh(apiKey, credential).finally(() => inFlight.delete(apiKey)),
    );
  return inFlight.get(apiKey)!;
}

/** The credential is on the person's desktop: every call is relayed there, never to a chosen host. */
const onDesktop = (llm: LlmConfig, apiKey: string, browserId?: string): LlmConfig => ({
  ...llm,
  baseUrl: LLM_DEFAULTS.chatgpt.base,
  transport: desktopRelay(apiKey, browserId),
});

/**
 * Whether `llm` holds the key's own saved credential. A key the request brought
 * (`new Oya({ model })`) is an API key for the catalog's endpoint, never a stored
 * sign-in to refresh or relay, whatever its text looks like.
 */
const savedOwn = (llm: LlmConfig) => !!llm.own && !requestLlm()?.apiKey;

/**
 * The LLM settings a request runs with, as resolve() gives them, except that a key
 * whose stored credential is a ChatGPT sign-in gets a live access token and always
 * the plan's own endpoint. Decided by the stored value, never by llm_provider or
 * openai_base_url: either of those can be changed by any administrator, and a
 * refresh token sent as a bearer to a chosen host would be that account's for good.
 */
export async function resolveFresh(apiKey: string, browserId?: string): Promise<LlmConfig> {
  // A model the request picked is checked here, where every model call passes and auth has settled the key.
  checkRequestLlm(() => resolveSaved(apiKey));
  const llm = resolve(apiKey);
  if (savedOwn(llm) && desktopHeld(llm.openaiKey)) return onDesktop(llm, apiKey, browserId);
  const stored = savedOwn(llm) ? credentialOf(llm.openaiKey) : null;
  return stored ? withLiveToken(llm, apiKey, stored) : llm;
}

/** `llm` with a live ChatGPT access token for the `stored` sign-in, and the plan's own endpoint. */
async function withLiveToken(llm: LlmConfig, apiKey: string, stored: Credential): Promise<LlmConfig> {
  // A hosted deployment never spends a plan, whatever a tenant pasted: OpenAI allows that for approved apps only.
  if (hosted()) return { ...llm, openaiKey: '' };
  const credential = live.get(apiKey)?.client_id === stored.client_id ? live.get(apiKey)! : stored;
  const { access_token } = await fresh(apiKey, credential);
  return { ...llm, openaiKey: access_token || '', baseUrl: LLM_DEFAULTS.chatgpt.base };
}
