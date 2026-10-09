/** Page-only automatic attachment from native tab ownership, without pausing renderers or discovering workers. */
import type { NativeBackend, NativeTarget } from './types.ts';
/** Automatic sessions have explicit ownership and a transport-level resource failure path. */
export interface AutoAttachDeps {
  /** Same protected target adapter used by ordinary page commands. */
  backend: NativeBackend;
  /** Check shared manual and automatic session capacity before allocating anything. */
  capacity(additional: number): void;
  /** Create and announce a flat session for an exact live native target. */
  attach(target: NativeTarget): string;
  /** Remove only a session owned by this automatic attacher. */
  detach(sessionId: string, targetId: string): void;
  /** Close a failed external transport rather than silently dropping future attachments. */
  fatal(reason: string): void;
}
/** One socket owns its own automatic sessions and native change subscription. */
export class NativeAutoAttach {
  /** Native dependencies contain no debugger or upstream protocol connection. */
  private readonly deps: AutoAttachDeps;
  /** Only automatic sessions appear here; manual attachment is never consumed by disable. */
  private readonly owned = new Map<string, string>();
  /** Native listener cancellation, absent while disabled. */
  private stop?: () => void;
  /** Construct connection-local ownership. */
  constructor(deps: AutoAttachDeps) {
    this.deps = deps;
  }
  /** Initial failures roll back; repeated enable does not allocate duplicate sessions. */
  set(enabled: boolean): object {
    if (!enabled) {
      this.dispose();
      return {};
    }
    if (this.stop) return {};
    this.start();
    return {};
  }
  /** Watch first, then reconcile an authorized snapshot; renderer pause is never requested. */
  private start(): void {
    if (!this.deps.backend.watchTargets) throw Error('Native automatic attachment is unavailable');
    this.stop = this.deps.backend.watchTargets(() => this.changed());
    try {
      this.refresh();
    } catch (error) {
      this.dispose();
      throw error;
    }
  }
  /** Future resource or ownership failures cannot disappear inside a browser event callback. */
  private changed(): void {
    try {
      this.refresh();
    } catch {
      this.dispose();
      this.deps.fatal('Native auto-attachment failed; reconnect after resolving target capacity');
    }
  }
  /** Reconcile exact target ids, checking total capacity before any new session allocation. */
  private refresh(): void {
    const targets = this.deps.backend.targets();
    const ids = new Set(targets.map((target) => target.targetId));
    for (const [id, session] of this.owned) if (!ids.has(id)) this.release(id, session);
    const added = targets.filter((target) => !this.owned.has(target.targetId));
    this.deps.capacity(added.length);
    for (const target of added) this.owned.set(target.targetId, this.deps.attach(target));
  }
  /** Forget ownership before emitting detach, making reentrant cleanup harmless. */
  private release(target: string, session: string): void {
    this.owned.delete(target);
    this.deps.detach(session, target);
  }
  /** Explicit client detach relinquishes automatic ownership without deleting a different session. */
  forget(sessionId: string): void {
    for (const [target, session] of this.owned) if (session === sessionId) this.owned.delete(target);
  }
  /** Disabling or disconnecting revokes automatic sessions but preserves independently attached sessions. */
  dispose(): void {
    this.stop?.();
    this.stop = undefined;
    for (const [target, session] of [...this.owned]) this.release(target, session);
  }
}
