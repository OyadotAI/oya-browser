/** Ephemeral contexts reuse native permission/protection setup and never copy the person's cookie jar. */
import { randomUUID } from 'node:crypto';
import type { Session } from 'electron';
import { CONTEXT_LIMIT } from './constants.ts';
import { ownSession, sessionVisible, sessionContext } from './registry.ts';
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
  /** Permanent disconnection flag fences asynchronous setup. */
  private closed = false;
  /** Application dependencies. */
  private readonly deps: ContextDependencies;
  /** Capture application policy and ownership callbacks. */
  constructor(deps: ContextDependencies) {
    this.deps = deps;
  }
  /** Return only contexts created by this authenticated connection. */
  list(): string[] {
    return [...this.sessions.keys()];
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
  /** Failed setup removes the context and clears any partial native session state. */
  private async prepare(id: string, session: Session): Promise<string> {
    try {
      await this.deps.configure(session);
      if (this.closed) throw Error('Native context owner disconnected');
      return id;
    } catch (error) {
      await this.failed(id, session);
      throw error;
    }
  }
  /** Cleanup also covers policy hooks installed after disconnection already revoked the id. */
  private async failed(id: string, session: Session): Promise<void> {
    if (this.sessions.has(id)) return this.remove(id);
    this.deps.close(session);
    await clearSession(session);
  }
  /** Exact owner validation precedes every tab, download or storage operation. */
  get(id: string): Session {
    const session = this.sessions.get(id);
    if (this.closed || !session) throw Error('Unknown or foreign native browser context');
    return session;
  }
  /** Cross-socket discovery never reveals another agent's private context tabs. */
  visible(session: Session): boolean {
    const id = sessionContext(session, this);
    return sessionVisible(session, this) && (!id || this.sessions.has(id));
  }
  /** Attach context metadata only for live owned contexts. */
  id(session: Session): string | undefined {
    return sessionContext(session, this);
  }
  /** Revocation happens before closing pages or awaiting any native cleanup. */
  async remove(id: string): Promise<void> {
    const session = this.sessions.get(id);
    if (!session) throw Error('Unknown or foreign native browser context');
    this.sessions.delete(id);
    this.deps.close(session);
    await clearSession(session);
  }
  /** Disconnect always destroys ephemeral contexts; callers cannot silently retain private renderers. */
  async dispose(): Promise<void> {
    this.closed = true;
    const results = await Promise.allSettled([...this.sessions.keys()].map((id) => this.remove(id)));
    if (results.some((result) => result.status === 'rejected')) throw Error('Native context cleanup failed');
  }
}
/** Close persistent connections as well as cache, cookies, service workers and storage. */
async function clearSession(session: Session): Promise<void> {
  await session.closeAllConnections();
  await session.clearStorageData();
  await session.clearCache();
}
