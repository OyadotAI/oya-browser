/**
 * `oya login`: save an API key for this machine, pasted or minted by signing
 * in with email and password. The key is proved against the control plane
 * before it is written.
 */
import { save, resolved, configPath } from '../config.ts';
import { ask, askSecret, choose } from '../prompt.ts';
import { flagStr, type Flags } from '../args.ts';

/** The sign-in answer. */
interface Session {
  /** A bearer token for the account. */
  access_token?: string;
  /** Why sign-in failed. */
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
}

/** POSTs JSON and returns the response with its parsed body. */
async function postJson<T>(url: string, body: unknown, token?: string): Promise<JsonReply<T>> {
  const headers = { ...(token ? { Authorization: `Bearer ${token}` } : {}), 'Content-Type': 'application/json' };
  const res = await fetch(url, { method: 'POST', headers, body: JSON.stringify(body) });
  return { ok: res.ok, body: (await res.json()) as T };
}

/** Signs in with email and password, then mints a key labelled "CLI" for this machine. */
async function mintKey(baseUrl: string): Promise<string> {
  const email = await ask('Email:');
  const password = await askSecret('Password:');
  const session = await postJson<Session>(`${baseUrl}/api/auth/login`, { email, password });
  if (!session.ok || !session.body.access_token) throw new Error(session.body.error || 'Sign-in failed');
  const minted = await postJson<Minted>(`${baseUrl}/api/auth/keys`, { label: 'CLI' }, session.body.access_token);
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

/** Prove it works before writing it — a saved-but-wrong key is a bad first run. */
async function verifyKey(baseUrl: string, apiKey: string): Promise<void> {
  const check = await fetch(`${baseUrl}/api/config`, { headers: { Authorization: `Bearer ${apiKey}` } });
  if (!check.ok) throw new Error(`That key was rejected by ${baseUrl} (${check.status})`);
}

/** The control plane: `--url`, else the current one when a key was passed, else asked. Trailing slashes dropped. */
async function loginUrl(flags: Flags): Promise<string> {
  const current = resolved().baseUrl;
  const url = flagStr(flags, 'url') || (flagStr(flags, 'key') ? current : await ask('Control plane URL:', current));
  return url.replace(/\/+$/, '');
}

/** `oya login [--key K] [--url U]`. With both flags it never prompts: that is the CI path. */
export async function cmdLogin(flags: Flags): Promise<void> {
  const baseUrl = await loginUrl(flags);
  const apiKey = await obtainKey(flags, baseUrl);
  if (!apiKey) throw new Error('No API key given');
  await verifyKey(baseUrl, apiKey);
  save({ apiKey, baseUrl });
  console.log(`\n✅ Signed in to ${baseUrl}. Saved to ${configPath}`);
  console.log('   Next: `oya init` to pick your model and browser provider.');
}
