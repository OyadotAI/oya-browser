/** Shell-only local imports validate selected IDs inside the library-import service. */
import type { AppServices } from '../app/services.ts';
import type { HandlersOf } from './handle.ts';

/** Private windows cannot import another browser's persistent browsing library. */
export class LibraryImportHandlers {
  /** The normal-profile import service and invoking window. */
  private readonly deps: Pick<AppServices, 'libraryImport' | 'tabs'>;
  /** Existing shell guard still authorizes both calls. */
  readonly handlers: HandlersOf<'library-import-sources' | 'import-library'> = {
    'library-import-sources': () => (this.deps.tabs.privateSession ? [] : this.deps.libraryImport.sources()),
    'import-library': (_event, source, profile) => this.import(source, profile),
  };
  /** Each window receives its own private-window refusal. */
  constructor(deps: Pick<AppServices, 'libraryImport' | 'tabs'>) {
    this.deps = deps;
  }
  /** Reject unchecked payloads before source discovery or persistent writes. */
  private import(source: string, profile: string) {
    if (this.deps.tabs.privateSession) throw Error('Import is available in a regular Oya window.');
    if (typeof source !== 'string' || typeof profile !== 'string') throw Error('Choose a browser and profile.');
    return this.deps.libraryImport.import(source, profile);
  }
}
