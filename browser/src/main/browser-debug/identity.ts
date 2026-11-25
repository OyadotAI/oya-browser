/** Native application/session owners supply metadata even when renderer measurement fails. */
import { createHash, randomUUID } from 'node:crypto';
import type { Session } from 'electron';
import type { NativePage } from '../native/index.ts';
import { BROWSER_DEBUG as C } from '../../shared/browser-debug.ts';
import { debugRegistry } from './registry.ts';
/** Storage paths never leave the browser process. */
interface Profile {
  /** Opaque native-session identity. */ id: string;
  /** Native persona label. */ label: string;
}
/** Native application APIs are injected by the composition root. */
interface Application {
  /** Running name. */ name(): string;
  /** Running version. */ version(): string;
  /** Running engine. */ engine(): string;
}
const profiles = new WeakMap<Session, Profile>();
let application: Application | undefined;
/** Bind native APIs, avoiding environment or installed-package guesses. */
export function useDebugApplication(source: Application): void {
  application = source;
}
/** Associate the label once with the immutable session that actually owns the tab. */
export function rememberDebugProfile(session: Session, label: string): void {
  if (profiles.has(session)) return;
  const path = debugStoragePath(session);
  const id = path ? createHash('sha256').update(path).digest('hex').slice(0, C.PROFILE_ID_CHARS) : randomUUID();
  profiles.set(session, { id, label: label.slice(0, C.MAX_LABEL) });
}
/** Native identity remains truthful if the page is frozen, closing or unregistered. */
export function debugNativeIdentity(page: NativePage) {
  return {
    contacted: true,
    browser: application?.name() ?? null,
    version: application?.version() ?? null,
    engine: application?.engine() ?? null,
    profile: debugNativeProfile(page),
    dpr: null as number | null,
  };
}
/** DPR is measured in the exact isolated document and is unknown when it cannot answer. */
export async function debugIdentity(page: NativePage) {
  const identity = debugNativeIdentity(page);
  try {
    identity.dpr = await measureDebugDpr(page);
  } catch {
    /* Native fields remain available. */
  }
  return identity;
}
/** Metadata requests use the same short deadline and document fence as inspection. */
async function measureDebugDpr(page: NativePage): Promise<number> {
  const registry = debugRegistry(page),
    doc = await registry.document(page.webContents.mainFrame);
  const metrics = await registry.run<{ /** Observed device pixel ratio. */ dpr: number }>(doc, 'metadata');
  if (!Number.isFinite(metrics.dpr) || metrics.dpr <= 0) throw Error('Page DPR unavailable');
  return metrics.dpr;
}

/** Unavailable optional metadata must not prevent a native tab from opening. */
function debugStoragePath(session: Session): string | null {
  try {
    return session.getStoragePath();
  } catch {
    return null;
  }
}

/** Closing a renderer makes its profile unknown without erasing native application identity. */
function debugNativeProfile(page: NativePage): Profile | null {
  try {
    const session = page.webContents.session;
    rememberDebugProfile(session, session.isPersistent() ? 'Persistent profile' : 'Private session');
    return profiles.get(session) ?? null;
  } catch {
    return null;
  }
}
