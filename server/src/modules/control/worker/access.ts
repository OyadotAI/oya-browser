/**
 * Credential re-checks: every gateway session, browser and stream viewer on
 * this replica is re-authenticated, and the ones that no longer pass are
 * disconnected.
 */
import { authenticateToken } from '../../auth/service.ts';
import { registry } from '../../browsers/registry.ts';
import { sessions as gateways } from '../../gateway/service.ts';
import { Status } from '../../../platform/http-status.ts';
import { exclusive } from './state.ts';
import { CloseCode } from './constants.ts';

/** Why a connection is closed. */
const REASON = 'Credential revoked or validation unavailable';

/** Re-authenticates every gateway session, browser and stream viewer on this replica and disconnects the ones whose credential no longer passes. */
export async function validateAttachments() {
  await exclusive('validating', () => closeRevoked(verdicts()));
}

/**
 * One check per credential per round. A revoked credential closes with 4003
 * (clients stop); an outage uses 1013 so desktop browsers reconnect.
 */
function verdicts() {
  const seen = new Map();
  return (token, allowBrowser = false) => {
    if (!seen.has(token)) seen.set(token, verdict(token, allowBrowser));
    return seen.get(token);
  };
}

/** Resolves null when the token passes, else the close code to use. */
const verdict = (token, allowBrowser) =>
  authenticateToken(token, { allowBrowser }).then(
    () => null,
    (e) => (e.status === Status.UNAVAILABLE ? CloseCode.TRY_AGAIN_LATER : CloseCode.REVOKED),
  );

/** Checks gateway sessions, then browsers and their viewers. */
async function closeRevoked(closeCode) {
  await Promise.all([...gateways.values()].map((session) => checkGateway(session, closeCode)));
  await Promise.all([...registry.browsers.entries()].map(([id, browser]) => checkBrowser(id, browser, closeCode)));
}

/** Closes a gateway client whose credential fails. */
async function checkGateway(session, closeCode) {
  if (await closeCode(session.authToken || session.apiKey)) session.client?.close(CloseCode.POLICY, REASON);
}

/** Closes a browser (and ends its viewers) whose credential fails, then checks each viewer's own credential. */
async function checkBrowser(browserId, browser, closeCode) {
  const code = await closeCode(browser.authToken || browser.apiKey, true);
  if (code) {
    browser.ws?.close(code, REASON);
    for (const viewer of browser.streamViewers) viewer.end();
  }
  await Promise.all([...browser.streamViewers].map((viewer) => checkViewer(browserId, browser, viewer, closeCode)));
}

/** Ends and removes a viewer whose credential fails. */
async function checkViewer(browserId, browser, viewer, closeCode) {
  if (await closeCode(viewer.authToken || browser.apiKey)) {
    viewer.end();
    registry.removeViewer(browserId, viewer);
  }
}
