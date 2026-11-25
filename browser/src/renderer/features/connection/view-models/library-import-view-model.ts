/** Explicit source/profile selection keeps local library import separate from signed-in-session transfer. */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { LibraryImportSource, LibraryImportResult } from '../../../../shared/ipc.ts';

/** Every import state remains visible and retryable without a server connection. */
interface State {
  /** Discovery result, with no local filesystem paths. */
  sources: LibraryImportSource[];
  /** Chosen browser. */
  source: string;
  /** Explicit profile choice; empty until selected. */
  profile: string;
  /** Discovery is still outstanding. */
  loading: boolean;
  /** One import is in flight. */
  busy: boolean;
  /** Safe discovery/read/persistence failure. */
  error: string;
  /** Accepted-entry counts and partial results. */
  note: string;
}
/** Only shell-owned local operations are exposed to this model. */
export type LibraryImportBridge = Pick<OyaBrowser, 'libraryImportSources' | 'importLibrary'>;
/** The renderer never displays raw filesystem exceptions. */
const READ_ERROR = 'Could not read this profile. Check file permissions and try again, or choose another profile.';

/** Counts describe retained additions, including truthful empty and partial outcomes. */
function resultNote(result: LibraryImportResult): string {
  const note = `Imported ${result.bookmarks} bookmarks and ${result.history} history entries.`;
  const extra = result.warnings.length
    ? result.warnings.join(' ')
    : !result.bookmarks && !result.history
      ? 'No new entries were added. Choose another profile if needed.'
      : '';
  return `${note}${extra ? ` ${extra}` : ''}`;
}

/** A changing source cannot apply a late result to another selection. */
export class LibraryImportViewModel extends ViewModel<State> {
  /** Typed shell calls preserve the selected-profile validation boundary. */
  private readonly bridge: LibraryImportBridge;
  /** Last request wins if discovery is explicitly retried. */
  private generation = 0;

  /** Lists sources without importing anything or selecting all profiles. */
  constructor(bridge: LibraryImportBridge) {
    super({ sources: [], source: '', profile: '', loading: true, busy: false, error: '', note: '' });
    this.bridge = bridge;
    void this.load();
  }

  /** Discovery failures are different from finding no supported data. */
  async load(): Promise<void> {
    const generation = ++this.generation;
    try {
      await this.discover(generation);
    } catch {
      if (generation === this.generation) this.set({ error: 'Could not check browser profiles. Try again.' });
    } finally {
      if (generation === this.generation) this.set({ loading: false });
    }
  }

  /** A stale discovery cannot replace a newer profile selection. */
  private async discover(generation: number): Promise<void> {
    this.set({ loading: true, error: '' });
    const sources = await this.bridge.libraryImportSources();
    if (generation === this.generation)
      this.set({ sources: sources || [], source: sources?.[0]?.id || '', profile: '' });
  }

  /** Source changes require a fresh deliberate profile selection. */
  chooseSource(source: string): void {
    if (!this.state.busy) this.set({ source, profile: '', error: '', note: '' });
  }

  /** The picker exposes only IDs main has listed; main rechecks them before reading. */
  chooseProfile(profile: string): void {
    if (!this.state.busy) this.set({ profile, error: '', note: '' });
  }

  /** Imports one selected profile and reports additions actually retained by the library. */
  async start(): Promise<void> {
    if (this.state.busy || !this.state.profile) return;
    this.set({ busy: true, error: '', note: '' });
    await this.perform().finally(() => this.set({ busy: false }));
  }

  /** A failed read keeps the selected profile ready for a deliberate retry. */
  private async perform(): Promise<void> {
    try {
      this.set({ note: resultNote(await this.bridge.importLibrary(this.state.source, this.state.profile)) });
    } catch {
      this.set({ error: READ_ERROR });
    }
  }
}
