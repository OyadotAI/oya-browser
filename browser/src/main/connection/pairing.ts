/**
 * A deep link is attacker-reachable input: any page the user visits can set
 * location.href to an oya:// URL. Connecting hands the target server this
 * browser's cookies, so an unattended handler would be drive-by cookie
 * exfiltration. Two things stop that:
 *
 *   1. The link carries a single-use pairing code, never a key, and the code is
 *      exchanged over HTTPS with the server it names. A code from a hostile page
 *      only redeems against that page's own server.
 *   2. The person is asked, with the destination host spelled out and Cancel as
 *      the default. A code proves the dashboard issued the link; it does not
 *      prove the user meant to click it.
 */
import type { MessageBoxOptions, MessageBoxReturnValue } from 'electron';
import { PAIRING_TIMEOUT_MS } from './constants.ts';

/** Shows a message box and answers the person's choice: Electron's dialog.showMessageBox, bound to a window. */
export type Ask = (options: MessageBoxOptions) => Promise<Answer>;

/** The person's choice: the button, and whether the checkbox was left ticked. */
type Answer = Pick<MessageBoxReturnValue, 'response'> & Partial<Pick<MessageBoxReturnValue, 'checkboxChecked'>>;

/** What the person agreed to: connecting, and whether to import their logins too. */
interface Confirmed {
  /** Whether to import the usual browser's logins. */
  importLogins: boolean;
}

/** A claim's outcome: the redeemed code, or why it failed. */
interface ClaimOutcome {
  /** The key and persona, when the code was redeemed. */
  ok?: Claimed;
  /** Why it was not. */
  error?: Error;
}

/** A usable pairing link: its code, and the server it names. */
interface PairingLink {
  /** The single-use pairing code. */
  code: string;
  /** The server's WebSocket URL. */
  parsed: URL;
}

/** What the server answers for a code: the key and persona, or why not. */
interface ClaimAnswer {
  /** The project key. */
  apiKey?: string;
  /** The persona to run as. */
  persona?: string;
  /** Why the code was refused. */
  error?: string;
}

/** A redeemed code: the key and the persona to run as. */
interface Claimed {
  /** The project key. */
  apiKey: string;
  /** The persona to run as. */
  persona: string;
}

/** A finished pairing: the key, persona and server, and whether to import the person's logins. */
export interface Paired extends Claimed, Confirmed {
  /** The server's WebSocket URL. */
  serverUrl: string;
}

/** Hosts a plaintext ws:// server may be on: this machine only. */
const LOCAL_HOSTS = ['localhost', '127.0.0.1', '::1', '[::1]'];

/** The pairing code and the server it names, or null when the link is not a usable oya:// link. */
function parsePairingLink(rawUrl: string): PairingLink | null {
  const url = URL.parse(rawUrl);
  if (!url || url.protocol !== 'oya:') return null;
  const code = url.searchParams.get('code');
  const server = url.searchParams.get('server');
  if (!code || !server) return null;
  const parsed = pairingServer(server);
  return parsed && { code, parsed };
}

/** The server URL, if it is ws:// or wss://. */
export function pairingServer(server: string): URL | null {
  const parsed = URL.parse(server);
  if (!parsed || !['ws:', 'wss:'].includes(parsed.protocol)) return null;
  // Plaintext ws:// is only reasonable against your own machine; anywhere else
  // it would put the key and every synced cookie on the wire in the clear.
  if (parsed.protocol === 'ws:' && !LOCAL_HOSTS.includes(parsed.hostname)) return null;
  return parsed;
}

/** The confirmation dialog, less the host it names. Cancel is the default. */
const PAIRING_PROMPT: Omit<MessageBoxOptions, 'message'> = {
  type: 'warning',
  buttons: ['Cancel', 'Connect'],
  defaultId: 0,
  cancelId: 0,
  title: 'Connect this browser?',
  detail:
    'This browser will sign in to that control plane and share its cookies and ' +
    'logged-in sessions with it, so remote browsers can act as you.\n\n' +
    'Only continue if you started this from that dashboard or your agent. Cancel if a web page opened it.',
  // An agent pairs the app to use the person's own logins, so bringing them over is the default.
  checkboxLabel: 'Also import my logins from my usual browser',
  checkboxChecked: true,
};

/** Asks the person to confirm, naming the host: `{ importLogins }` for Connect, null otherwise. */
async function confirmPairing(ask: Ask, parsed: URL): Promise<Confirmed | null> {
  const { response, checkboxChecked } = await ask({ ...PAIRING_PROMPT, message: `Connect to ${parsed.host}?` });
  return response === 1 ? { importLogins: !!checkboxChecked } : null;
}

/** The claim request: JSON, no redirects, bounded in time. */
function pairingClaimRequest(code: string): RequestInit {
  return {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    redirect: 'error',
    body: JSON.stringify({ code }),
    signal: AbortSignal.timeout(PAIRING_TIMEOUT_MS),
  };
}

/** Redeems the code with the server for a key and persona; throws with the server's reason. */
async function claimPairingCode(parsed: URL, code: string): Promise<Claimed> {
  const claimUrl = `${parsed.protocol === 'wss:' ? 'https' : 'http'}://${parsed.host}/api/pairing/claim`;
  const res = await fetch(claimUrl, pairingClaimRequest(code));
  const body = (await res.json().catch(() => ({}))) as ClaimAnswer;
  if (!res.ok || !body.apiKey) throw new Error(body.error || `Pairing failed (${res.status})`);
  return { apiKey: body.apiKey, persona: body.persona || 'default' };
}

/** The claim as `{ ok }` or `{ error }`, never a rejection. */
function tryClaimPairingCode(parsed: URL, code: string): Promise<ClaimOutcome> {
  return claimPairingCode(parsed, code).then(
    (ok) => ({ ok }),
    (error: Error) => ({ error }),
  );
}

/** Pairs from an oya:// link: `{ apiKey, persona, serverUrl, importLogins }`, or false when refused or failed. */
export async function pairFromLink(rawUrl: string, ask: Ask): Promise<Paired | false> {
  const link = parsePairingLink(rawUrl);
  const confirmed = link && (await confirmPairing(ask, link.parsed));
  if (!link || !confirmed) return false;
  const claim = await tryClaimPairingCode(link.parsed, link.code);
  if (claim.ok) return { ...claim.ok, serverUrl: link.parsed.href, ...confirmed };
  await ask({ type: 'error', title: 'Could not pair', message: 'Pairing failed', detail: claim.error?.message });
  return false;
}
