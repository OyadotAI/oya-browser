/**
 * `oya login`: save an API key for this machine, pasted or minted by signing
 * in with email and password. The key is proved against the control plane
 * before it is written.
 */
import { save, resolved, configPath } from '../config.ts';
import { ask, askSecret, choose } from '../prompt.ts';
import { flagStr, type Flags } from '../args.ts';
import { Oya, type OyaError } from '@oya-ai/browser';
import { out } from '../context.ts';
import { CliError, usage } from '../errors.ts';
import { LOGIN_TIMEOUT_MS, Status } from '../constants.ts';

/** The sign-in answer. */
interface Session {
  /** A bearer token for the account. */
  access_token?: string;
  /** Sent with the token, so the server can step the session up after an authenticator code. */
  refresh_token?: string;
  /** True when the account has an authenticator and the session still needs its code. */
  mfa_required?: boolean;
  /** Why sign-in failed. */
  error?: string;
}

/** One authenticator on the account. */
interface Factor {
  /** Its id, which the code is checked against. */
  id: string;
  /** `verified` once it is set up. */
  status: string;
}

/** The account's authenticators, as the MFA status answer lists them. */
interface MfaStatus {
  /** Each authenticator and whether it is set up. */
  factors?: Factor[];
  /** Why the status could not be read. */
  error?: string;
}

/** The key-minting answer. */
interface Minted {
  /** The new API key. */
  key?: string;
  /** Why it could not be created. */
  error?: string;
}

/** A response's status and parsed body. */
interface JsonReply<T> {
  /** Whether the status was 2xx. */
  ok: boolean;
  /** The parsed body. */
  body: T;
  /** The cookies it set, as a Cookie header sends them back: the server keeps a sign-in's refresh token there. */
  cookies: string;
}

/** The headers that say who is asking: the bearer token, and the cookie the server set at sign-in. */
const credentials = (token?: string, cookie = '') => ({
  ...(token ? { Authorization: `Bearer ${token}` } : {}),
  ...(cookie ? { Cookie: cookie } : {}),
});

/** POSTs JSON and returns the response with its parsed body. */
async function postJson<T>(url: string, body: unknown, token?: string, cookie = ''): Promise<JsonReply<T>> {
  const headers = { ...credentials(token, cookie), 'Content-Type': 'application/json' };
  const signal = AbortSignal.timeout(LOGIN_TIMEOUT_MS);
  const res = await fetch(url, { method: 'POST', headers, body: JSON.stringify(body), signal }).catch(() => {
    throw new CliError(`Could not reach ${new URL(url).origin}.`, 'unreachable', {
      hint: 'Check the address, then run oya login again.',
    });
  });
  return { ok: res.ok, body: jsonOr(await res.text(), `${res.status} from ${url}`) as T, cookies: cookiesOf(res) };
}

/** The cookies a response set, as a Cookie header sends them back. */
const cookiesOf = (res: Response) =>
  res.headers
    .getSetCookie()
    .map((c) => c.split(';')[0])
    .join('; ');

/** The body as JSON, or an error naming the answer when it is not (a proxy's HTML page). */
function jsonOr(text: string, answer: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return { error: `Not a JSON answer: ${answer}` };
  }
}

/** The id of the account's set-up authenticator, asked of the server with the signed-in session. */
async function verifiedFactor(baseUrl: string, session: JsonReply<Session>, body: object): Promise<string> {
  const url = `${baseUrl}/api/auth/mfa/status`;
  const status = await postJson<MfaStatus>(url, body, session.body.access_token, session.cookies);
  const factor = status.body.factors?.find((f) => f.status === 'verified');
  if (!factor) throw new Error(status.body.error || 'No authenticator app found for this account');
  return factor.id;
}

/** The token to mint with: the signed-in one, or, for an account with an authenticator, the one its code steps up to. */
async function steppedUp(baseUrl: string, session: JsonReply<Session>): Promise<string> {
  if (!session.body.mfa_required) return session.body.access_token as string;
  const body = { refresh_token: session.body.refresh_token };
  const factorId = await verifiedFactor(baseUrl, session, body);
  const asked = { ...body, factor_id: factorId, code: await ask('Code from your authenticator app:') };
  const verify = `${baseUrl}/api/auth/mfa/verify`;
  const verified = await postJson<Session>(verify, asked, session.body.access_token, session.cookies);
  if (!verified.ok || !verified.body.access_token) throw new Error(verified.body.error || 'That code did not work');
  return verified.body.access_token;
}

/** Signs in with email and password, and the authenticator code when the account has one; the bearer token. */
async function signIn(baseUrl: string): Promise<string> {
  const email = await ask('Email:');
  const password = await askSecret('Password:');
  const session = await postJson<Session>(`${baseUrl}/api/auth/login`, { email, password });
  if (!session.ok || !session.body.access_token) throw new Error(session.body.error || 'Sign-in failed');
  return steppedUp(baseUrl, session);
}

/** Signs in, then mints a key labelled "CLI" for this machine. */
async function mintKey(baseUrl: string): Promise<string> {
  const minted = await postJson<Minted>(`${baseUrl}/api/auth/keys`, { label: 'CLI' }, await signIn(baseUrl));
  if (!minted.ok || !minted.body.key) throw new Error(minted.body.error || 'Could not create an API key');
  console.log('  Created a new API key labelled "CLI".');
  return minted.body.key;
}

/** How to authenticate, when no key was passed. */
function chooseMethod(): Promise<string> {
  return choose('How do you want to authenticate?', [
    { id: 'paste', label: 'Paste an API key', note: 'from the dashboard, or a self-hosted key' },
    { id: 'password', label: 'Sign in with email and password', note: 'mints a new key for this machine' },
  ]);
}

/** The key: from `--key`, pasted, or minted by signing in. */
async function obtainKey(flags: Flags, baseUrl: string): Promise<string> {
  const given = flagStr(flags, 'key') || '';
  if (given) return given;
  const how = await chooseMethod();
  if (how === 'paste') return askSecret('API key:');
  return mintKey(baseUrl);
}

/**
 * Prove it works before writing it: a saved-but-wrong key is a bad first run.
 * Through the SDK, so a dead server or a rejected key reads as it does for
 * every other command, with the same time limit.
 */
async function verifyKey(baseUrl: string, apiKey: string): Promise<void> {
  // The address and key may have been typed at the prompt, which fail() cannot see: name them here.
  await new Oya({ apiKey, baseUrl }).config.get().catch((err) => {
    throw namedFailure(err, baseUrl);
  });
}

/** A failed check, naming the address typed: unreachable, or the key refused; anything else as it came. */
function namedFailure(err: unknown, baseUrl: string): unknown {
  const status = (err as OyaError).status;
  const again = 'then run oya login again.';
  if (status === 0)
    return new CliError(`Could not reach ${baseUrl}.`, 'unreachable', { hint: `Check the address, ${again}` });
  if (status !== Status.UNAUTHORIZED && status !== Status.FORBIDDEN) return err;
  return new CliError(`${baseUrl} rejected that key.`, 'invalid_key', { hint: `Check the key, ${again}` });
}

/** The control plane: `--url`, else the current one when a key was passed, else asked. Trailing slashes dropped. */
async function loginUrl(flags: Flags): Promise<string> {
  const current = resolved().baseUrl;
  const url = flagStr(flags, 'url') || (flagStr(flags, 'key') ? current : await ask('Control plane URL:', current));
  return url.replace(/\/+$/, '');
}

/** `oya login [--key K] [--url U]`. With both flags it never prompts: that is the CI path. */
export async function cmdLogin(flags: Flags): Promise<void> {
  if (flags.json && !flagStr(flags, 'key')) throw usage('oya login --json never prompts, so it needs --key <key>.');
  const baseUrl = await loginUrl(flags);
  const apiKey = await obtainKey(flags, baseUrl);
  if (!apiKey) throw usage('No API key given.');
  await verifyKey(baseUrl, apiKey);
  save({ apiKey, baseUrl });
  out(flags, { ok: true, baseUrl, configPath }, () => signedIn(baseUrl));
}

/** What a person reads after signing in: where, where it is saved, and the next step. */
function signedIn(baseUrl: string): void {
  console.log(`\n✅ Signed in to ${baseUrl}. Saved to ${configPath}`);
  console.log('   Scripts using @oya-ai/browser read this file too, so `new Oya()` needs no environment.');
  console.log('   Next: `oya init` to pick your model and browser provider.');
}
