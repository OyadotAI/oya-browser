/** Ephemeral contexts reuse native permission/protection setup and never copy the person's cookie jar. */
import { randomUUID } from 'node:crypto';
import type { Session } from 'electron';
import { CONTEXT_LIMIT } from './constants.ts';
import { ownSession, sessionVisible, sessionContext } from './registry.ts';
import { retireContext } from './cleanup.ts';
/** Application seams keep policy setup and tab destruction browser-owned. */
export interface ContextDependencies {
  /** Create an in-memory native partition, never a persistent profile. */
  create(partition: string): Session;
  /** Apply current persona constraints, egress and sensitive permission policy before pages exist. */
  configure(session: Session): Promise<void>;
  /** Close all tabs and popups with this exact native session. */
  close(session: Session): void;
}
/** One authenticated connection owns every context it creates until explicit disposal or disconnect. */
export class NativeContexts {
  /** Per-connection session namespace; absent ids are never looked up globally. */
  private readonly sessions = new Map<string, Session>();
  /** Context ids become discoverable only after their full asynchronous setup succeeds. */
  private readonly exposed = new Set<string>();
  /** Permanent disconnection flag fences asynchronous setup. */
  private closed = false;
  /** Application dependencies. */
  private readonly deps: ContextDependencies;
  /** Capture application policy and ownership callbacks. */
  constructor(deps: ContextDependencies) {
    this.deps = deps;
  }
  /** Return only fully configured contexts created by this authenticated connection. */
  list(): string[] {
    return this.closed ? [] : [...this.exposed];
  }
  /** Native creation is bounded and protection completes before publishing its identity. */
  async create(): Promise<string> {
    if (this.closed || this.sessions.size >= CONTEXT_LIMIT) throw Error('Native context limit or closed owner');
    const id = randomUUID(),
      session = this.deps.create(`oya-native-context-${id}`);
    ownSession(session, this, id);
    this.sessions.set(id, session);
    return this.prepare(id, session);
  }
  /** Failed setup revokes the context and retires resources; immutable native policy is never reset. */
  private async prepare(id: string, session: Session): Promise<string> {
    try {
      await this.deps.configure(session);
      return this.publish(id, session);
    } catch (error) {
      return this.rejectSetup(id, session, error);
    }
  }
  /** Publication and liveness checking happen synchronously after the last setup await. */
  private publish(id: string, session: Session): string {
    if (this.closed || this.sessions.get(id) !== session)
      throw Error('Native context owner disconnected or context cancelled');
    this.exposed.add(id);
    return id;
  }
  /** Preserve the setup failure even when retiring its partial state fails too. */
  private async rejectSetup(id: string, session: Session, error: unknown): Promise<never> {
    try {
      await this.failed(id, session);
    } catch (cleanup) {
      throw new AggregateError([error, cleanup], 'Native context setup and cleanup failed', { cause: cleanup });
    }
    throw error;
  }
  /** Cleanup also covers policy hooks installed after disconnection already revoked the id. */
  private async failed(id: string, session: Session): Promise<void> {
    if (this.sessions.has(id)) return this.remove(id);
    await retireContext(session, (owned) => this.deps.close(owned));
  }
  /** Exact owner validation precedes every tab, download or storage operation. */
  get(id: string): Session {
    const session = this.sessions.get(id);
    if (this.closed || !session || !this.exposed.has(id)) throw Error('Unknown or foreign native browser context');
    return session;
  }
  /** Cross-socket discovery never reveals another agent's private context tabs. */
  visible(session: Session): boolean {
    const id = sessionContext(session, this);
    return !this.closed && sessionVisible(session, this) && (!id || this.exposed.has(id));
  }
  /** Attach context metadata only for live owned contexts. */
  id(session: Session): string | undefined {
    const id = sessionContext(session, this);
    return !this.closed && id && this.exposed.has(id) ? id : undefined;
  }
  /** Revocation happens before closing pages or awaiting any native cleanup. */
  async remove(id: string): Promise<void> {
    const session = this.sessions.get(id);
    if (!session) throw Error('Unknown or foreign native browser context');
    this.exposed.delete(id);
    this.sessions.delete(id);
    await retireContext(session, (owned) => this.deps.close(owned));
  }
  /** Disconnect revokes all contexts and attempts every disposal; cleanup failures are reported. */
  async dispose(): Promise<void> {
    this.closed = true;
    const results = await Promise.allSettled([...this.sessions.keys()].map((id) => this.remove(id)));
    if (results.some((result) => result.status === 'rejected')) throw Error('Native context cleanup failed');
  }
}
