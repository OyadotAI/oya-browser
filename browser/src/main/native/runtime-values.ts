/** Connection-owned value routing keeps child-frame handles out of sibling documents. */
import { RUNTIME, type RuntimeContext, type RuntimeReply } from './runtime-types.ts';
/** Native handles carry exact document ownership and their engine release group. */
interface ValueOwner {
  /** Exact document that allocated this native value. */
  context: RuntimeContext;
  /** Group inherited by native inspection and receiver calls. */
  group: string;
}
/** Only native descriptor object IDs are indexed; JSON-by-value page data is never treated as a handle. */
function descriptors(reply: RuntimeReply): Record<string, unknown>[] {
  const properties = (reply.properties || []) as Record<string, object>[];
  return [
    reply.result,
    reply.exception?.exception,
    ...properties.flatMap((p) => [p.value, p.get, p.set, p.symbol]),
  ].filter((value): value is Record<string, unknown> => !!value);
}
/** A bounded routing index, not a second JavaScript object registry. */
export class RuntimeValues {
  /** Opaque engine identities are never guessed or resolved through page globals. */
  private readonly values = new Map<string, ValueOwner>();
  /** Select a receiver only if this connection and exact target own it. */
  owner(target: string, params: Record<string, unknown>): ValueOwner | undefined {
    const id = params.objectId ?? params.promiseObjectId;
    if (id === undefined) return;
    const owner = this.values.get(String(id));
    if (!owner || owner.context.target !== target) throw Error('Stale or foreign native value');
    return owner;
  }
  /** Cross-frame value arguments are rejected before any page function executes. */
  validate(context: RuntimeContext, params: Record<string, unknown>): void {
    for (const arg of (params.arguments || []) as Record<string, unknown>[]) {
      if (arg.objectId === undefined) continue;
      const owner = this.values.get(String(arg.objectId));
      if (owner?.context.uniqueId !== context.uniqueId) throw Error('Stale or foreign native argument context');
    }
  }
  /** Remember only successful engine descriptors, preserving inherited release groups. */
  remember(context: RuntimeContext, group: string, reply: RuntimeReply): void {
    this.prune(context);
    const ids = descriptors(reply).flatMap((d) => (typeof d.objectId === 'string' ? [d.objectId] : []));
    if (this.values.size + ids.length > RUNTIME.handles) throw Error('Native value routing limit exceeded');
    for (const id of ids) this.values.set(id, { context, group });
  }
  /** Released objects and groups cannot route later calls even before the next native read. */
  release(target: string, params: Record<string, unknown>): void {
    for (const [id, owner] of this.values)
      if (owner.context.target === target && (id === params.objectId || owner.group === params.objectGroup))
        this.values.delete(id);
  }
  /** Replacement documents cannot keep their old routing entries alive. */
  private prune(context: RuntimeContext): void {
    for (const [id, owner] of this.values)
      if (
        owner.context.frame.detached ||
        (owner.context.frame === context.frame &&
          (owner.context.mainDocument ?? owner.context.document) !== (context.mainDocument ?? context.document))
      )
        this.values.delete(id);
  }
  /** Disconnect retains no target or frame references. */
  clear(): void {
    this.values.clear();
  }
}
