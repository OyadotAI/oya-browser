/** CDP target discovery consumes native ownership changes, never an engine debugging target list. */
import type { NativeBackend, NativeTarget } from './types.ts';
/** Serialized target information available to one authenticated client. */
export interface DiscoveredTarget extends NativeTarget {
  /** Whether this connection currently has a session for the target. */
  attached: boolean;
}
/** Discovery dependencies preserve connection-local session identity and event delivery. */
export interface DiscoveryDeps {
  /** Browser-owned native target registry. */
  backend: NativeBackend;
  /** Current connection's attachment state. */
  attached(target: string): boolean;
  /** Closed native targets invalidate local sessions. */
  removed(target: string): void;
  /** Only this authenticated socket receives the discovery events. */
  emit(method: string, params: Record<string, unknown>): void;
}
/** A subscriber snapshots only authorized targets and releases all native listeners on disable/disconnect. */
export class NativeDiscovery {
  /** Dependencies cannot include a debugger or protocol forwarding function. */
  private readonly deps: DiscoveryDeps;
  /** Last state actually delivered to this connection, never a global cache. */
  private known = new Map<string, DiscoveredTarget>();
  /** Native listener cancellation, absent while disabled. */
  private stop?: () => void;
  /** Bind discovery to the same native backend as this connection's page commands. */
  constructor(deps: DiscoveryDeps) {
    this.deps = deps;
  }
  /** Repeated enable is idempotent; disabled connections retain no tab observers. */
  set(enabled: boolean): object {
    if (!enabled) {
      this.dispose();
      return {};
    }
    if (this.stop) return {};
    this.start();
    return {};
  }
  /** Roll back a failed initial native snapshot rather than leaving a half-enabled observer. */
  private start(): void {
    if (!this.deps.backend.watchTargets) throw Error('Native target discovery is unavailable');
    this.stop = this.deps.backend.watchTargets(() => this.refresh());
    try {
      this.refresh();
    } catch (error) {
      this.dispose();
      throw error;
    }
  }
  /** Reconcile native snapshots without announcing unchanged tab metadata repeatedly. */
  refresh(): void {
    if (!this.stop) return;
    const current = new Map(this.deps.backend.targets().map((target) => [target.targetId, this.info(target)]));
    const previous = this.known;
    this.known = current;
    for (const [id] of previous) if (!current.has(id)) this.removed(id);
    for (const [id, target] of current) this.updated(previous.get(id), target);
  }
  /** Attachment metadata is owned by the connection, not inferred from page state. */
  private info(target: NativeTarget): DiscoveredTarget {
    return { ...target, attached: this.deps.attached(target.targetId) };
  }
  /** Destroyed or revoked targets cannot leave stale sessions usable by future commands. */
  private removed(targetId: string): void {
    this.deps.removed(targetId);
    this.deps.emit('Target.targetDestroyed', { targetId });
  }
  /** Emit exactly the native change that happened, with no synthetic navigation or worker events. */
  private updated(previous: DiscoveredTarget | undefined, targetInfo: DiscoveredTarget): void {
    if (JSON.stringify(previous) === JSON.stringify(targetInfo)) return;
    this.deps.emit(previous ? 'Target.targetInfoChanged' : 'Target.targetCreated', { targetInfo });
  }
  /** Drop the listener and metadata cache; subsequent enable reports a fresh authorized snapshot. */
  dispose(): void {
    this.stop?.();
    this.stop = undefined;
    this.known.clear();
  }
}
