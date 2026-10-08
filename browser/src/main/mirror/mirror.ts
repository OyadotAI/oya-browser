/**
 * Mirroring the user's real browser into Oya: capture every profile's device
 * and cookies, hand them to the server as personas, seed the desktop's own
 * partition with the site storage, then reconnect as the mirrored persona so
 * the desktop and its remote browsers are already signed in. Runs once, on the
 * first authenticated connection, and again on an explicit re-import.
 */
import type { AppServices } from '../app/services.ts';
import { captureAll, type Capture, type CaptureDeps, type ProfileCapture } from './capture.ts';
import { seedStorage, type StorageElectron } from './storage.ts';
import { installedSources, type ListedSource } from './locate.ts';
import { MIRROR_ANSWER_TIMEOUT_MS, IMPORTS_KEPT } from './constants.ts';

/** What the mirror uses of the app: settings, the socket, the shell, and Electron. */
type Deps = Pick<AppServices, 'config' | 'socket' | 'shell' | 'electron'>;

/** The two steps that touch the machine, replaceable in tests. */
export interface MirrorSteps {
  /** Captures the browser named `sourceId` (the default one when omitted). */
  capture?: (ctx: CaptureDeps, sourceId?: string) => Promise<Capture | null>;
  /** Copies a profile's site storage into a persona's partition. */
  seed?: (electron: StorageElectron, personaId: string, userDataDir: string, profileDir: string) => void;
}

/** The server's mirror_ok: the personas it made, in capture order, and the one to use. */
export interface MirrorOk {
  /** One persona id per captured profile. */
  personaIds?: string[];
  /** The persona the desktop switches to. */
  defaultPersonaId: string;
}

/** The server's mirror_failed. */
export interface MirrorFailed {
  /** Why it refused the import. */
  error?: string;
}

/** What the renderer is told about an import in progress or finished. */
type Status = Record<string, unknown>;

/** One finished import, as the account page lists it. */
interface ImportRecord {
  /** The source browser's name. */
  source: string;
  /** How many profiles came over. */
  profiles: number;
  /** How many cookies came over. */
  cookies: number;
  /** How many distinct sites those cookies are for. */
  sites: number;
  /** When it finished, in epoch milliseconds. */
  at?: number;
}

/** The mirror_persona message: one entry per captured profile, all on the real device. */
function mirrorMessage(captured: Capture) {
  return { type: 'mirror_persona', profiles: captured.profiles.map((p) => profilePayload(captured, p)) };
}

/** One profile as the server needs it: its identity, the shared real device, and its session state. */
function profilePayload(captured: Capture, p: ProfileCapture) {
  const identity = { source: captured.source, profile: p.profile, name: `${captured.name} · ${p.name}` };
  return { ...identity, lastUsed: p.lastUsed, device: captured.device, cookies: p.cookies, origins: {} };
}

/** Shown when an import is asked for with no server to import into. */
const NOT_CONNECTED = 'Connect to Oya first, then import your logins.';
/** Shown when the server never answers an import. */
const NO_ANSWER = 'The server did not answer. Check your connection and try again.';

/** How many profiles, cookies and sites (distinct cookie domains) a capture holds, for the person to read. */
function tally(captured: Capture): ImportRecord {
  const all = captured.profiles.flatMap((p) => p.cookies || []);
  const sites = new Set(all.map((c) => String(c.domain || '').replace(/^\./, ''))).size;
  return { source: captured.name, profiles: captured.profiles.length, cookies: all.length, sites };
}

/** The import history with `record` first, at most IMPORTS_KEPT long. */
const withImport = (imports: unknown, record: ImportRecord): unknown[] =>
  [record, ...(Array.isArray(imports) ? imports : [])].slice(0, IMPORTS_KEPT);

/** Drives one mirror run and applies the server's answer. Wired in as ctx.mirror. */
export class Mirror {
  /** The app services the mirror reads and drives. */
  private readonly ctx: Deps;
  /** Captures the real browser. */
  private readonly capture: NonNullable<MirrorSteps['capture']>;
  /** Seeds a persona's partition with site storage. */
  private readonly seed: NonNullable<MirrorSteps['seed']>;
  /** The capture waiting for the server's mirror_ok, or null. */
  private pending: Capture | null = null;
  /** Gives up on a server that never answers. */
  private answerTimer: ReturnType<typeof setTimeout> | undefined = undefined;
  /** Set when the person ticked "import my logins" while pairing: import once the new connection signs in. */
  importOnConnect = false;
  /** Capture and server acknowledgment form one exclusive import operation. */
  private running = false;

  /** `ctx` is the app's services; `steps` replaces the capture and the storage seeding (tests). */
  constructor(ctx: Deps, { capture = captureAll, seed = seedStorage }: MirrorSteps = {}) {
    this.ctx = ctx;
    this.capture = capture;
    this.seed = seed;
  }

  /**
   * Runs the mirror once, on first sign-in, when it is turned on. Off by
   * default: a successful mirror reconnects to switch personas, which nobody
   * asked for at sign-in. The person starts it themselves from "Import logins"
   * (reimport), where the reconnect is the expected result, or by ticking it
   * while pairing. OYA_MIRROR=1 turns the automatic run on.
   */
  maybeRun(): Promise<void> | undefined {
    if (this.importOnConnect) {
      this.importOnConnect = false;
      return this.reimport();
    }
    const { mirroredFrom, apiKey } = this.ctx.config.values;
    if (process.env.OYA_MIRROR !== '1' || mirroredFrom || !apiKey) return undefined;
    return this.run();
  }

  /** The browsers on this machine an import can read, the default one first. */
  sources(): ListedSource[] {
    return installedSources();
  }

  /** Imports `sourceId` (the default browser when omitted) from scratch; the device stays fixed server-side. */
  reimport(sourceId?: string): Promise<void> {
    this.ctx.config.merge({ mirroredFrom: '' });
    return this.run(sourceId);
  }

  /** Captures the real browser and sends it to the server. Every way it can end is told to the person. */
  async run(sourceId?: string): Promise<void> {
    if (!this.ctx.socket.ready || !this.ctx.socket.isOpen()) return this.notify({ done: true, error: NOT_CONNECTED });
    if (this.running) return;
    this.running = true;
    this.notify({ started: true });
    const captured = await this.capture(this.ctx, sourceId).catch((e: unknown) => e);
    if (captured instanceof Error) return this.failed(captured.message);
    if (!captured) return this.done('none', { empty: true });
    this.send(captured as Capture);
  }

  /** Hands the capture to the server and waits, not for ever, for its answer. */
  private send(captured: Capture): void {
    this.pending = captured;
    this.ctx.socket.send(mirrorMessage(captured));
    this.answerTimer = setTimeout(() => this.failed(NO_ANSWER), MIRROR_ANSWER_TIMEOUT_MS);
  }

  /** The import did not happen: say why, and leave it free to be tried again. */
  private failed(error: string): void {
    console.log('[oya] Mirror failed:', error);
    this.settle();
    this.notify({ done: true, error });
  }

  /** Forgets the capture in flight and stops waiting for its answer. */
  private settle(): Capture | null {
    this.running = false;
    clearTimeout(this.answerTimer);
    const captured = this.pending;
    this.pending = null;
    return captured;
  }

  /** The server made the personas: seed the default's storage, remember it and the import, and reconnect as it. */
  onOk(msg: MirrorOk): void {
    const captured = this.settle();
    if (!captured) return;
    this.seedDefault(captured, msg);
    const record = this.remember(captured);
    this.ctx.config.merge({ mirroredFrom: captured.source, persona: msg.defaultPersonaId });
    this.ctx.config.save();
    this.reconnect();
    this.notify({ done: true, ...record, ...(captured.warnings?.length ? { warnings: captured.warnings } : {}) });
  }

  /** Puts this import at the top of the history the account page shows; answers its record. */
  private remember(captured: Capture): ImportRecord {
    const record = { ...tally(captured), at: Date.now() };
    this.ctx.config.merge({ imports: withImport(this.ctx.config.values.imports, record) });
    return record;
  }

  /** The server refused the import: tell the person why. */
  onFailed(msg: MirrorFailed): void {
    this.failed(msg.error || 'The server refused the import.');
  }

  /** Copies the default persona's site storage into its partition before it is loaded. */
  private seedDefault(captured: Capture, msg: MirrorOk): void {
    const at = (msg.personaIds || []).indexOf(msg.defaultPersonaId);
    const profile = captured.profiles[at];
    if (!profile) return;
    try {
      this.seed(this.ctx.electron, msg.defaultPersonaId, captured.userDataDir, profile.profile);
    } catch (e) {
      (captured.warnings ??= []).push(`Site storage could not be copied: ${(e as Error).message}`);
    }
  }

  /** Drops the socket and dials again, so the next auth runs as the mirrored persona. */
  private reconnect(): void {
    this.ctx.socket.disconnect();
    this.ctx.socket.connect();
  }

  /** Marks the run finished so it does not repeat, and notifies the renderer. */
  private done(mirroredFrom: string, status: Status): void {
    this.running = false;
    this.ctx.config.merge({ mirroredFrom });
    this.ctx.config.save();
    this.notify({ done: true, ...status });
  }

  /** Tells the renderer where the import is, for the setup and profile screens. */
  private notify(status: Status): void {
    this.ctx.shell.send('mirror-status', status);
  }
}
