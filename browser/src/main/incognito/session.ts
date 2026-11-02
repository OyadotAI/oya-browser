/**
 * The one in-memory session every incognito window shares. Created by the
 * first incognito window, protected exactly like an agent context (persona
 * proxy, egress and page policy), and wiped when the last incognito window
 * closes. Owning it in the native-context registry keeps it out of history,
 * session restore and CDP discovery without more checks.
 */
import { randomUUID } from 'node:crypto';
import type { DownloadItem, Event, Session } from 'electron';
import type { AppServices } from '../app/services.ts';
import { protectPrivateSession } from '../app/native-contexts.ts';
import { ownSession, retireContext } from '../native-contexts/index.ts';
import type { SessionExtras } from '../identity/session.ts';
import { inContainer } from '../shell/hold-still.ts';
import { INCOGNITO_PARTITION } from './constants.ts';

/** Every session this process ever made for incognito, so a tab's session says what kind it is. */
const incognitoSessions = new WeakSet<Session>();
/** The registry owner no agent connection can match, so incognito pages are never agent-visible. */
const INCOGNITO_OWNER = Object.freeze({});

/** Whether a page lives in an incognito session: no history, console, recording or cookie pull for it. */
export function isIncognito(session: Session | undefined): boolean {
  return !!session && incognitoSessions.has(session);
}

/** Incognito leaves nothing on disk: a download is refused before any save dialog or file. */
function refuseDownload(event: Event, item: DownloadItem): void {
  event.preventDefault();
  item.cancel();
}

/** Owns the shared incognito session's lifetime. */
export class IncognitoSession {
  /** Session setup and protection services. */
  private readonly deps: AppServices;
  /** The session being opened or open; concurrent windows wait on the same one. */
  private pending?: Promise<Session>;
  /** `mailto:` and other app links still ask the person, as they do in a normal window. */
  private readonly externalApp: SessionExtras['externalApp'] = (url, contents) =>
    void this.deps.externalApps.request(url, contents);
  /** Reads the services lazily, so it can be built before the persona exists. */
  constructor(deps: AppServices) {
    this.deps = deps;
  }
  /** A governed or headless cloud browser has no person to browse privately, so it never offers one. */
  available(): boolean {
    return !this.deps.governance.configuration && !inContainer();
  }
  /** The protected session; refused when unavailable, or when it was retired while this call waited. */
  async get(): Promise<Session> {
    if (!this.available()) throw Error('Incognito windows are not available in this browser');
    const pending = (this.pending ??= this.open());
    const session = await pending.catch((error) => this.forget(pending, error));
    if (this.pending !== pending) throw Error('The incognito session closed while the window opened');
    return session;
  }
  /** A failed open lets the next window try again, but never drops a newer session that replaced it meanwhile. */
  private forget(failed: Promise<Session>, error: unknown): never {
    if (this.pending === failed) this.pending = undefined;
    throw error;
  }
  /**
   * Drops the session and wipes its cookies, storage and cache; the next
   * incognito window gets a fresh one. ponytail: Electron keeps the emptied
   * Session object alive for the rest of the run, one per incognito generation.
   */
  async retire(): Promise<void> {
    const pending = this.pending;
    this.pending = undefined;
    const session = await pending?.catch(() => undefined);
    if (session) await retireContext(session, () => {}).catch((error) => console.error('[incognito]', error));
  }
  /** A fresh in-memory partition, marked before any renderer, then protected. */
  private async open(): Promise<Session> {
    const session = this.create();
    await protectPrivateSession(this.deps, session, { externalApp: this.externalApp });
    return session;
  }
  /** The partition, owned in the native-context registry and marked incognito before anything renders in it. */
  private create(): Session {
    const id = randomUUID();
    const session = this.deps.electron.session.fromPartition(INCOGNITO_PARTITION + id, { cache: false });
    ownSession(session, INCOGNITO_OWNER, id);
    incognitoSessions.add(session);
    session.on('will-download', refuseDownload);
    return session;
  }
}
