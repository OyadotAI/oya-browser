/** Media requests bind to their actual window and session rather than the application's selected target. */
import type { Session, WebContents } from 'electron';
import type { AppServices } from './services.ts';

/** Window-local selection is distinct from the shared profile identity. */
type Scope = Pick<AppServices, 'tabs' | 'shell'>;
/** Production has a registry; isolated single-window fixtures may supply their own concrete services. */
export type MediaOwnerServices = Pick<AppServices, 'windows' | 'tabs' | 'shell' | 'persona'>;
/** An approval cannot follow a tab into another window, session or profile. */
export interface MediaBinding {
  /** Exact owning window scope, never the global service router. */ owner: Scope | undefined;
  /** Native session identity includes private/profile separation. */ session: Session;
  /** Persona changes invalidate an outstanding approval. */ partition: string;
}
/** A registered requester resolves through the native ownership registry, failing closed when absent. */
export function mediaOwner(deps: MediaOwnerServices, contents: WebContents): Scope | undefined {
  const owner = deps.windows ? deps.windows.ownerOfContents(contents) : deps;
  return owner?.tabs.privateSession || owner?.shell.window?.isDestroyed() ? undefined : owner;
}
/** Capture before queueing or prompting so transfer cannot silently rebind consent. */
export function mediaBinding(deps: MediaOwnerServices, contents: WebContents): MediaBinding {
  return { owner: mediaOwner(deps, contents), session: contents.session, partition: deps.persona.partitionName() };
}
/** Native ownership and profile identity must remain equal across every asynchronous consent step. */
export function sameMediaBinding(deps: MediaOwnerServices, contents: WebContents, before: MediaBinding): boolean {
  const now = mediaBinding(deps, contents);
  return now.owner === before.owner && now.session === before.session && now.partition === before.partition;
}
