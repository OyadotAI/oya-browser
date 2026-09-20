/** Which replica owns a session or attachment. */
import { control, instanceId, projectId, terminal } from '../service.ts';

/** The live replica that owns this key's session or attachment, or null when this replica should serve it. */
export async function ownerFor(id, key) {
  const [[session], [attachment]] = await control().store.load([
    { kind: 'session', id },
    { kind: 'attachment', id },
  ]);
  const s = session?.body || attachment?.body;
  if (servedHere(s, key)) return null;
  const owner = await control().store.get('instance', s.instance);
  return owner?.url && owner.leaseUntil >= Date.now() ? owner : null;
}

/** Terminal or orphaned sessions (for example, owned by a replaced process) are served from durable state wherever the request lands. */
function servedHere(s, key) {
  return (
    !s ||
    s.project !== projectId(key) ||
    s.instance === instanceId ||
    terminal.has(s.state) ||
    s.leaseUntil < Date.now()
  );
}
