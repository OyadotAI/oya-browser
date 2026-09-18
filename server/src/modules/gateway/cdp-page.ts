/**
 * A second CDP connection to a session's browser, attached to its first page.
 * Profiles and recordings observe the browser this way so the client's own
 * wire is never touched.
 */
import { CDPConnection } from '../../drivers/cdp.ts';

/** Connects to `upstreamUrl` and attaches to its first page; null (connection closed) when there is no page. */
export async function openPage(upstreamUrl) {
  const conn: any = await new CDPConnection(upstreamUrl).connect();
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
