/**
 * "Imported logins" on the account page: bring the sessions of a browser on
 * this computer (Chrome, Firefox, Arc, Brave, Edge) into Oya, so nobody has to
 * sign in to every site again. The main process does the import and keeps the
 * last few (config `imports`); this holds the latest, the ones before it, the
 * choice of browser, and how it went.
 */
import { ViewModel } from '../../../core/view-model.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { ConnectionStatus } from '../../../../shared/ipc.ts';
import { DEFAULT_IMPORT_BROWSERS, TEXT } from '../model/constants.ts';
import { messageOf, shape, type ImportRecord, type ImportSource, type MirrorStatus } from '../model/models.ts';

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
}

/** The parts of the bridge imports use. */
export type ImportBridge = Pick<OyaBrowser, 'importSources' | 'reimportBrowser' | 'onMirrorStatus' | 'onWsStatus'>;

/** How much one import brought: sites when it counted them, else cookies. */
export function importAmount(record: ImportRecord): string {
  const [count, what] = record.sites ? [record.sites, 'site'] : [record.cookies || 0, 'cookie'];
  return `${count} ${what}${count === 1 ? '' : 's'}`;
}

/** No import yet: what an import is for, naming the browsers this computer has. */
export function emptyImportText(sources: ImportSource[] | null): string {
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

/** The status line: what stands in the way or, when nothing does, the last note. */
export function importStatus(state: ImportState): ImportNote {
  const blocked = importBlocked(state);
  return blocked ? { text: blocked, kind: '' } : state.note;
}

/** Imports earlier than the latest that the page lists. */
export const earlierImports = (state: ImportState): ImportRecord[] =>
  state.history.slice(1, C.EARLIER_IMPORTS_SHOWN + 1);

/** Importing logins from another browser. */
export class ImportViewModel extends ViewModel<ImportState> {
  /** The main process. */
  private readonly bridge: ImportBridge;

  /** No history, offline, then lists the browsers on this computer and follows imports and the connection. */
  constructor(bridge: ImportBridge) {
    super({ connected: false, running: false, history: [], sources: null, chosen: '', note: { text: '', kind: '' } });
    this.bridge = bridge;
    this.own(bridge.onWsStatus((status) => this.onStatus(status)));
    this.own(bridge.onMirrorStatus((status) => this.onMirror(shape<MirrorStatus>(status))));
    void this.load();
  }

  /** Lists the browsers found on this computer, choosing the first (the person's default). */
  async load(): Promise<void> {
    const listed = await this.bridge.importSources().catch(() => []);
    const sources = (listed ?? []).map((source) => shape<ImportSource>(source));
    this.set({ sources, chosen: sources[0]?.id ?? '' });
  }

  /** Takes the import history the main process kept. */
  showHistory(imports: unknown): void {
    this.set({ history: Array.isArray(imports) ? imports.map((r) => shape<ImportRecord>(r)) : [] });
  }

  /** The connection came or went. A successful import reconnects, and its summary outlives that. */
  onStatus(status: ConnectionStatus): void {
    this.set({ connected: !!status.connected });
  }

  /** A browser was chosen in the list. */
  choose(id: string): void {
    this.set({ chosen: id });
  }

  /** Starts the import of the chosen browser; its progress arrives through onMirrorStatus. */
  async start(): Promise<void> {
    const name = this.state.sources?.find((s) => s.id === this.state.chosen)?.name ?? 'your browser';
    this.set({ running: true, note: { text: `Reading your logins from ${name}…`, kind: '' } });
    await this.bridge.reimportBrowser(this.state.chosen).catch((e: unknown) => this.finish(messageOf(e), 'error'));
  }

  /** Progress from the main process: started, or done with a result, an error, or nothing to bring. */
  private onMirror(status: MirrorStatus): void {
    if (status.started) return this.set({ running: true });
    if (status.error) return this.finish(status.error, 'error');
    if (status.empty) return this.finish(TEXT.importEmpty);
    const record = { ...status, source: status.source ?? '', at: status.at || Date.now() };
    this.set({ history: [record, ...this.state.history].slice(0, C.EARLIER_IMPORTS_SHOWN + 1) });
    this.finish(TEXT.importDone);
  }

  /** The import ended: say how, and give the button back. */
  private finish(text: string, kind: ImportNote['kind'] = ''): void {
    this.set({ running: false, note: { text, kind } });
  }
}
