/**
 * "Imported logins" on the account page: bring the sessions of a browser on
 * this computer (Chrome, Safari, Firefox, Arc, Brave, Edge) into Oya, so nobody has to
 * sign in to every site again. The main process does the import and keeps the
 * last few (config `imports`); this holds the latest, the ones before it, the
 * choice of browser, and how it went.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { ConnectionStatus, PasswordImportResult } from '../../../../shared/ipc.ts';
import { DEFAULT_IMPORT_BROWSERS, TEXT } from '../model/constants.ts';
import { messageOf, shape, type ImportRecord, type ImportSource, type MirrorStatus } from '../model/models.ts';
import { LibraryImportViewModel, type LibraryImportBridge } from './library-import-view-model.ts';

/** A line under the import row; `kind` is 'error' for a failure. */
export interface ImportNote {
  /** The words. */
  text: string;
  /** '' or 'error'. */
  kind: '' | 'error';
}

/** What the import group knows. */
export interface ImportState {
  /** Whether the server is connected: an import needs somewhere to go. */
  connected: boolean;
  /** First-login invitation remains until accepted or dismissed. */
  offer: boolean;
  /** An import is running. */
  running: boolean;
  /** Finished imports, newest first. */
  history: ImportRecord[];
  /** The browsers on this computer, the default first; null until listed. */
  sources: ImportSource[] | null;
  /** The chosen browser's id. */
  chosen: string;
  /** The last thing to tell the person: progress, or how the last import went. Kept across reconnects. */
  note: ImportNote;
  /** Oya already opens web links from other apps, so Settings says so instead of offering Make default. */
  isDefault: boolean;
}

/** The parts of the bridge imports use. */
export type ImportBridge = LibraryImportBridge &
  Pick<
    OyaBrowser,
    | 'importSources'
    | 'reimportBrowser'
    | 'makeDefaultBrowser'
    | 'onboardingStatus'
    | 'importPasswords'
    | 'onMirrorStatus'
    | 'onWsStatus'
    | 'getUiPreferences'
    | 'saveUiPreferences'
  >;

/** How much one import brought: sites when it counted them, else cookies. */
export function importAmount(record: ImportRecord): string {
  const [count, what] = record.sites ? [record.sites, 'site'] : [record.cookies || 0, 'cookie'];
  return `${count} ${what}${count === 1 ? '' : 's'}`;
}

/** No import yet: what an import is for, naming the browsers this computer has. */
export function emptyImportText(sources: ImportSource[] | null): string {
  if (sources && !sources.length)
    return 'Sign in to your sites directly in Oya. Bookmarks and history can be imported separately above.';
  const names: readonly string[] = sources?.length ? sources.map((s) => s.name) : DEFAULT_IMPORT_BROWSERS;
  const list = names.length > 1 ? `${names.slice(0, -1).join(', ')} or ${names[names.length - 1]}` : names[0];
  return `Bring your logins from ${list}, so you are already signed in here.`;
}

/** A browser as the list offers it; the person's default says so. */
export const sourceLabel = (s: ImportSource): string => (s.isDefault ? `${s.name} (your default browser)` : s.name);

/** Whether the browser list is hidden: none were found on this computer. */
export const noSources = (state: ImportState): boolean => state.sources?.length === 0;

/** What stands in the way of an import, or '' when nothing does. */
export function importBlocked(state: ImportState): string {
  if (noSources(state)) return TEXT.importNoBrowser;
  return state.connected ? '' : TEXT.importOffline;
}

/**
 * The status line: being offline first, then the last note, then what stands in the way.
 * A Mac with only Safari has no browser to import from, but its passwords import still
 * has to say how it went.
 */
export function importStatus(state: ImportState): ImportNote {
  const blocked = importBlocked(state);
  const shown = blocked === TEXT.importOffline || (blocked && !state.note.text);
  return shown ? { text: blocked, kind: '' } : state.note;
}

/** Imports earlier than the latest that the page lists. */
export const earlierImports = (state: ImportState): ImportRecord[] =>
  state.history.slice(1, C.EARLIER_IMPORTS_SHOWN + 1);

/** Initial invitation and discovery state before any IPC replies. */
function initialImportState(): ImportState {
  const note: ImportNote = { text: '', kind: '' };
  const empty = { history: [], sources: null, chosen: '', note, isDefault: false };
  return { offer: false, connected: false, running: false, ...empty };
}

/** A partial import says what failed instead of silently claiming complete success. */
function importResult(status: MirrorStatus): string {
  if (status.warnings?.length) return `Imported with warnings: ${status.warnings.join(' ')}`;
  if (status.kept && status.cookies) return `Imported. Still browsing as ${status.kept}; switch from the profile menu.`;
  return status.cookies ? TEXT.importDone : 'No logins were found. You may need to sign in to your sites in Oya.';
}

/** What a passwords import says: how many it kept, and a reminder that the export file is plain text. */
export function passwordsResult({ imported, skipped }: PasswordImportResult): string {
  const kept = `Imported ${imported} password${imported === 1 ? '' : 's'}`;
  return `${kept}${skipped ? `, skipped ${skipped}` : ''}. Delete the exported file: it holds them unencrypted.`;
}

/** Importing logins from another browser. */
export class ImportViewModel extends ViewModel<ImportState> {
  /** Readable library data has a separate local-only transfer and consent. */
  readonly library: LibraryImportViewModel;
  /** The main process. */
  private readonly bridge: ImportBridge;
  /** Opens account settings when the first-login invitation is ready. */
  onOffer?: () => void;
  /** Closes the first-run offer's dialog when the person says Not now. */
  onDecline?: () => void;
  /** A loaded preference prevents reconnects from offering import again. */
  private offered = true;

  /** No history, offline, then lists the browsers on this computer and follows imports and the connection. */
  constructor(bridge: ImportBridge) {
    super(initialImportState());
    this.bridge = bridge;
    this.library = new LibraryImportViewModel(bridge);
    this.watch(bridge);
  }

  /** Owns the local importer and all events without interrupting setup on connection. */
  private watch(bridge: ImportBridge): void {
    this.own(() => this.library.dispose());
    this.own(bridge.onWsStatus((status) => this.onStatus(status)));
    this.own(bridge.onMirrorStatus((status) => this.onMirror(shape<MirrorStatus>(status))));
    void this.load();
    void this.loadOffer();
    void this.refreshDefault();
    this.watchFocus();
  }

  /** Lists the browsers found on this computer, choosing the first (the person's default). */
  async load(): Promise<void> {
    const listed = await this.bridge.importSources().catch(() => []);
    const sources = (listed ?? []).map((source) => shape<ImportSource>(source));
    this.set({ sources, chosen: sources[0]?.id ?? '' });
    this.offerIfReady();
  }

  /** Takes the import history the main process kept. */
  showHistory(imports: unknown): void {
    this.set({ history: Array.isArray(imports) ? imports.map((r) => shape<ImportRecord>(r)) : [] });
  }

  /** The connection came or went. A successful import reconnects, and its summary outlives that. */
  onStatus(status: ConnectionStatus): void {
    this.set({ connected: !!status.connected });
    this.offerIfReady();
  }

  /** A browser was chosen in the list. */
  choose(id: string): void {
    this.set({ chosen: id });
  }

  /** Starts the import of the chosen browser; its progress arrives through onMirrorStatus. */
  async start(): Promise<void> {
    const name = this.state.sources?.find((s) => s.id === this.state.chosen)?.name ?? 'your browser';
    if (this.state.running) return;
    void this.dismissOffer();
    this.set({ running: true, note: { text: `Reading your logins from ${name}…`, kind: '' } });
    await this.bridge.reimportBrowser(this.state.chosen).catch((e: unknown) => this.finish(messageOf(e), 'error'));
  }

  /** Asks the system to make Oya the default browser (the first-run offer's second step). */
  async makeDefault(): Promise<void> {
    await this.bridge.makeDefaultBrowser().catch(() => {});
    await this.refreshDefault();
  }

  /** Reads whether Oya is the default browser; an unanswered check counts as not, so the button stays. */
  async refreshDefault(): Promise<void> {
    const status = await this.bridge.onboardingStatus().catch(() => null);
    this.set({ isDefault: !!status?.isDefault });
  }

  /**
   * The OS confirms the choice after makeDefaultBrowser returns (a macOS prompt, Windows Settings),
   * so check again when the person comes back to the window.
   */
  private watchFocus(): void {
    const refresh = () => void this.refreshDefault();
    globalThis.addEventListener?.('focus', refresh);
    this.own(() => globalThis.removeEventListener?.('focus', refresh));
  }

  /** Imports a passwords export the person chooses, and says how it went. */
  async importPasswords(): Promise<void> {
    if (this.state.running) return;
    this.set({ running: true });
    const done = await this.bridge.importPasswords().catch((e: unknown) => this.finish(messageOf(e), 'error'));
    if (done) this.finish(passwordsResult(done));
    else this.set({ running: false });
  }

  /** Progress from the main process: started, or done with a result, an error, or nothing to bring. */
  private onMirror(status: MirrorStatus): void {
    if (status.started) return this.set({ running: true });
    if (status.error) return this.finish(status.error, 'error');
    if (status.empty) return this.finish(TEXT.importEmpty);
    const record = { ...status, source: status.source ?? '', at: status.at || Date.now() };
    this.set({ history: [record, ...this.state.history].slice(0, C.EARLIER_IMPORTS_SHOWN + 1) });
    this.finish(importResult(status));
  }

  /** Load onboarding independently from browser discovery; both can finish in either order. */
  private async loadOffer(): Promise<void> {
    const preferences = await this.bridge.getUiPreferences().catch(() => null);
    this.offered = preferences?.importOffered !== false && preferences?.importOffered !== undefined;
    if (preferences) this.offerIfReady();
  }

  /** Offer once after login, when a supported source was actually found. */
  private offerIfReady(): void {
    if (this.offered || !this.state.connected || !this.state.sources?.length) return;
    this.offered = true;
    this.set({ offer: true });
    // The optional invitation is shown when Settings is opened, never over a first task or setup guide.
  }

  /** Not now: the offer is remembered as made, and its dialog closes, back to the browser. */
  async notNow(): Promise<void> {
    this.onDecline?.();
    await this.dismissOffer();
  }

  /** Remember the person's choice without reconnecting or changing account settings. */
  async dismissOffer(): Promise<void> {
    this.set({ offer: false });
    await this.bridge.saveUiPreferences({ importOffered: true }).catch(() => false);
  }

  /** The import ended: say how, and give the button back. */
  private finish(text: string, kind: ImportNote['kind'] = ''): void {
    this.set({ running: false, note: { text, kind } });
  }
}
