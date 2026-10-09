/** Packaging must prove agent-owned runtime worlds, not just the older internal analyzer world. */
const OWNER = 'oya-packaging-probe';
const WORLD = 'isolated-packaging-probe';
/** Native runtime error replies fail packaging even when the underlying IPC promise resolves. */
async function command(frame, context, operation, params = {}) {
  const reply = await frame._runOyaRuntime(OWNER, context, operation, params);
  if (reply.error || reply.exception) throw Error('Native runtime world probe failed');
  return reply;
}
/** Exercise a private world, its shared DOM and separate globals, then revoke its native owner. */
async function inspectWorld(frame) {
  const main = (await command(frame, '', 'context')).context;
  const isolated = (await command(frame, main, 'isolatedContext', { world: WORLD })).context;
  if (!main || !isolated || main === isolated) throw Error('Native runtime world identities are not isolated');
  try {
    await inspectValues(frame, main, isolated);
  } finally {
    await command(frame, isolated, 'close', { world: WORLD });
    await command(frame, main, 'close');
  }
}
/** No page/preload privilege or cross-world global sharing may be introduced by a packaged engine. */
async function inspectValues(frame, main, isolated) {
  const source = 'globalThis.oyaPackagingSecret=42; typeof process + ":" + typeof require + ":" + typeof document';
  const privateReply = await command(frame, isolated, 'evaluate', { world: WORLD, source, byValue: true });
  const publicReply = await command(frame, main, 'evaluate', { source: 'typeof oyaPackagingSecret', byValue: true });
  if (privateReply.result?.value !== 'undefined:undefined:object' || publicReply.result?.value !== 'undefined')
    throw Error('Native runtime world isolation probe failed');
}
module.exports = { inspectWorld };
