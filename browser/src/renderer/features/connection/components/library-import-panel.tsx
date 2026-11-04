/** Import scope and an explicit profile choice appear before any local data is read. */
import { useViewModel } from '../../../hooks/index.ts';
import { Button } from '../../../ui/index.ts';
import type { LibraryImportViewModel } from '../view-models/library-import-view-model.ts';
import { SetupIllustration } from './setup-illustration.tsx';

/** Bookmarks/history import works offline and never claims protected sessions were transferred. */
export function LibraryImportPanel({ vm }: { /** Local-only import state. */ vm: LibraryImportViewModel }) {
  const s = useViewModel(vm);
  const source = s.sources.find((item) => item.id === s.source);
  return (
    <section className="library-import-panel" aria-label="Import from another browser">
      <SetupIllustration scene="import" />
      <h3>Bring bookmarks and history</h3>
      {source && !source.sessions && <p>Windows sessions can’t transfer. Sign in again in Oya.</p>}
      {s.loading ? (
        <p role="status">Looking for browser profiles…</p>
      ) : !s.sources.length ? (
        <p>No supported browser profiles found.</p>
      ) : (
        <div className="library-import-fields">
          <label>
            Browser
            <select value={s.source} disabled={s.busy} onChange={(e) => vm.chooseSource(e.target.value)}>
              {s.sources.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            Browser profile
            <select value={s.profile} disabled={s.busy} onChange={(e) => vm.chooseProfile(e.target.value)}>
              <option value="">Choose a profile</option>
              {source?.profiles.map((profile) => (
                <option value={profile.id} key={profile.id}>
                  {profile.name}
                </option>
              ))}
            </select>
          </label>
          <Button variant="secondary" disabled={s.busy || !s.profile} onClick={() => void vm.start()}>
            {s.busy ? `Reading ${source?.name}…` : 'Import bookmarks and history'}
          </Button>
        </div>
      )}
      {s.error && <p role="alert">{s.error}</p>}
      {s.note && <p role="status">{s.note}</p>}
      <Button disabled={s.loading || s.busy} onClick={() => void vm.load()}>
        Check again
      </Button>
    </section>
  );
}
