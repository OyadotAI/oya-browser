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
const { PAIRING_TIMEOUT_MS } = require('./constants.cjs');

/** Hosts a plaintext ws:// server may be on: this machine only. */
const LOCAL_HOSTS = ['localhost', '127.0.0.1', '::1', '[::1]'];

/** The pairing code and the server it names, or null when the link is not a usable oya:// link. */
function parsePairingLink(rawUrl) {
  const url = URL.parse(rawUrl);
  if (!url || url.protocol !== 'oya:') return null;
  const code = url.searchParams.get('code');
  const server = url.searchParams.get('server');
  if (!code || !server) return null;
  const parsed = pairingServer(server);
  return parsed && { code, parsed };
}

/** The server URL, if it is ws:// or wss://. */
function pairingServer(server) {
  const parsed = URL.parse(server);
  if (!parsed || !['ws:', 'wss:'].includes(parsed.protocol)) return null;
  // Plaintext ws:// is only reasonable against your own machine; anywhere else
  // it would put the key and every synced cookie on the wire in the clear.
  if (parsed.protocol === 'ws:' && !LOCAL_HOSTS.includes(parsed.hostname)) return null;
  return parsed;
}

/** The confirmation dialog, less the host it names. Cancel is the default. */
const PAIRING_PROMPT = {
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
async function confirmPairing(ask, parsed) {
  const { response, checkboxChecked } = await ask({ ...PAIRING_PROMPT, message: `Connect to ${parsed.host}?` });
  return response === 1 ? { importLogins: !!checkboxChecked } : null;
}

/** The claim request: JSON, no redirects, bounded in time. */
function pairingClaimRequest(code) {
  return {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    redirect: 'error',
    body: JSON.stringify({ code }),
    signal: AbortSignal.timeout(PAIRING_TIMEOUT_MS),
  };
}

/** Redeems the code with the server for a key and persona; throws with the server's reason. */
async function claimPairingCode(parsed, code) {
  const claimUrl = `${parsed.protocol === 'wss:' ? 'https' : 'http'}://${parsed.host}/api/pairing/claim`;
  const res = await fetch(claimUrl, pairingClaimRequest(code));
  const body = await res.json().catch(() => ({}));
  if (!res.ok || !body.apiKey) throw new Error(body.error || `Pairing failed (${res.status})`);
  return { apiKey: body.apiKey, persona: body.persona || 'default' };
}

/** The claim as `{ ok }` or `{ error }`, never a rejection. */
function tryClaimPairingCode(parsed, code) {
  return claimPairingCode(parsed, code).then(
    (ok) => ({ ok }),
    (error) => ({ error }),
  );
}

/** Pairs from an oya:// link: `{ apiKey, persona, serverUrl, importLogins }`, or false when refused or failed. */
async function pairFromLink(rawUrl, ask) {
  const link = parsePairingLink(rawUrl);
  const confirmed = link && (await confirmPairing(ask, link.parsed));
  if (!confirmed) return false;
  const claim = await tryClaimPairingCode(link.parsed, link.code);
  if (claim.ok) return { ...claim.ok, serverUrl: link.parsed.href, ...confirmed };
  await ask({ type: 'error', title: 'Could not pair', message: 'Pairing failed', detail: claim.error.message });
  return false;
}

module.exports = { pairFromLink };
