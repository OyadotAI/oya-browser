/**
 * A second CDP connection to a session's browser, attached to its first page.
 * Profiles and recordings observe the browser this way so the client's own
 * wire is never touched.
 */
import { CDPConnection } from '../../drivers/cdp.ts';
import type { CdpEndpoint } from '../browsers/driver/index.ts';

/**
 * Opens the endpoint and attaches to its first page; null (connection closed)
 * when there is no page. A failure after the socket opened closes it: the
 * caller never learns of a connection it did not get back.
 */
export async function openPage(endpoint: CdpEndpoint) {
  const conn: any = CDPConnection.over(await endpoint.open());
  try {
    return await attachFirstPage(conn);
  } catch (err) {
    conn.close();
    throw err;
  }
}

/** Attaches to the first page, or closes the connection and answers null when there is none. */
async function attachFirstPage(conn) {
  const page = await firstPage(conn);
  if (!page) {
    conn.close();
    return null;
  }
  const { sessionId } = await conn.send('Target.attachToTarget', { targetId: page.targetId, flatten: true });
  return { conn, sessionId };
}

/** The browser's first page target, if it has one. */
async function firstPage(conn) {
  const { targetInfos = [] } = await conn.send('Target.getTargets');
  return targetInfos.find((t) => t.type === 'page');
}
