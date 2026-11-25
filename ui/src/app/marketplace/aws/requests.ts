/** Setup requests accept only registration states, never inferring confirmation from identity resolution. */
import { apiUrl, authHeaders } from '@/lib/api';
import { forgetMarketplace } from '@/lib/marketplace';

/** Every buyer-visible state, including safe recovery without provider diagnostics. */
export type SetupKind =
  | 'loading'
  | 'signed-out'
  | 'ready'
  | 'pending'
  | 'missing'
  | 'expired'
  | 'conflict'
  | 'changed'
  | 'unavailable'
  | 'offline';

/** A response tied to the account token that requested it. */
export interface SetupViewState {
  /** Decides copy and allowed actions. */
  kind: SetupKind;
  /** Auth identity correlation prevents painting a previous account's response. */
  forToken: string | null;
  /** The trusted last four AWS account digits, when resolved. */
  awsAccount?: string;
  /** Binds the next explicit connection action to the purchase displayed. */
  contextId?: string;
  /** Keeps the current content visible during a repeat request. */
  busy: boolean;
  /** Only an explicit connection completion moves heading focus. */
  connected?: boolean;
}

/** A deliberate connection is bound to the context displayed by the page. */
export interface SetupConsent {
  /** Stable request identity for retries within the displayed setup. */
  key: string;
  /** Opaque nonce returned by the preceding context/status read. */
  contextId: string;
}

/** Maps fixed server codes to fixed copy, never displaying an internal exception. */
function failureKind(code: string): SetupKind {
  const kinds: Record<string, SetupKind> = {
    invalid_setup: 'missing',
    setup_expired: 'expired',
    purchase_conflict: 'conflict',
    setup_changed: 'changed',
  };
  return Object.hasOwn(kinds, code) ? kinds[code] : 'unavailable';
}

/** Rejects unexpected server states rather than accidentally declaring success. */
function settled(data: Record<string, unknown>, token: string | null): SetupKind {
  if (!['ready', 'pending'].includes(String(data.status))) return 'unavailable';
  if (!/^\d{4}$/.test(String(data.awsAccount))) return 'unavailable';
  if (!/^[A-Za-z0-9-]{16,128}$/.test(String(data.contextId))) return 'unavailable';
  return token ? (data.status as 'ready' | 'pending') : 'signed-out';
}

/** Creates a safe state from JSON and clears only a nonsecret navigation hint. */
function result(data: Record<string, unknown>, ok: boolean, token: string | null, connect: boolean): SetupViewState {
  const kind = ok ? settled(data, token) : failureKind(String(data.code));
  if (['missing', 'expired', 'pending'].includes(kind)) forgetMarketplace();
  const account = { forToken: token, busy: false, awsAccount: String(data.awsAccount || '') };
  return { ...account, kind, contextId: String(data.contextId || ''), connected: connect && kind === 'pending' };
}

/** The token belongs to the Oya session, while the trusted purchase stays in its HttpOnly cookie. */
export async function requestSetup(token: string | null, signal: AbortSignal, consent?: SetupConsent) {
  const path = consent ? '/connect' : token ? '/status' : '';
  const headers = setupHeaders(token, consent);
  const init = { signal, headers, ...(consent ? { method: 'POST', body: '{}' } : {}) };
  const response = await fetch(apiUrl(`/marketplace/aws${path}`), init);
  const data = await response.json().catch(() => ({}));
  return result(data, response.ok, token, Boolean(consent));
}

/** Only an explicit consent action carries a purchase-context binding. */
function setupHeaders(token: string | null, consent?: SetupConsent) {
  const headers = token ? authHeaders(token) : {};
  if (!consent) return headers;
  return {
    ...headers,
    'Idempotency-Key': consent.key,
    'X-Oya-Marketplace-Context': consent.contextId,
  };
}

/** Callback failures contain only enumerated nonsecret state names. */
export function callbackProblem(): SetupKind | null {
  const value = new URLSearchParams(window.location.search).get('setup');
  if (value === 'invalid') return 'missing';
  return value === 'unavailable' ? 'unavailable' : null;
}
