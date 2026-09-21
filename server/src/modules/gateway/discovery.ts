/**
 * Chrome's HTTP discovery endpoints, so CDP clients treat the gateway as a
 * browser.
 */
import { authenticateToken } from '../auth/service.ts';
import { Status } from '../../platform/http-status.ts';
import { sessions } from './session-store.ts';
import { BEARER_PREFIX_LENGTH, CHROME_VERSION } from './constants.ts';

/** The host the client reached us on, and the WebSocket scheme that matches it. */
function endpointOf(req) {
  const host = req.headers.host || 'localhost';
  const scheme = (req.headers['x-forwarded-proto'] || '').includes('https') ? 'wss' : 'ws';
  return { host, scheme };
}

/**
 * CDP discovery. Playwright and Puppeteer fetch this first and then dial
 * webSocketDebuggerUrl, which is why pointing them at the gateway just works.
 */
export function handleJsonVersion(req, res) {
  const { host, scheme } = endpointOf(req);
  const token = new URL(req.url, `http://${host}`).searchParams.get('token');
  res.json({
    ...CHROME_VERSION,
    webSocketDebuggerUrl: `${scheme}://${host}/connect${token ? `?token=${encodeURIComponent(token)}` : ''}`,
  });
}

/** Some clients probe /json/list before connecting. */
export async function handleJsonList(req, res) {
  const found = await principalOf(req, res);
  if (!found) return;
  const { host, scheme } = endpointOf(req);
  res.json([...sessions.values()].filter((s) => s.apiKey === found.principal.key).map((s) => listing(s, scheme, host)));
}

/** The caller's principal, or null after answering with the auth error. */
async function principalOf(req, res) {
  try {
    return { principal: await authenticateToken(req.headers.authorization?.slice(BEARER_PREFIX_LENGTH)) };
  } catch (e) {
    res.status(e.status || Status.UNAVAILABLE).json({ error: e.message });
    return null;
  }
}

/** One session as a /json/list page entry. */
function listing(s, scheme, host) {
  return {
    id: s.id,
    type: 'page',
    title: s.profile ? `Gateway session (${s.profile})` : 'Gateway session',
    url: 'about:blank',
    webSocketDebuggerUrl: `${scheme}://${host}/connect?session=${s.id}`,
  };
}
