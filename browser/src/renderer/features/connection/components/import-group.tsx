/**
 * Imports: on the account page, the rows of the Logins section (the latest import
 * or what one is for, the ones before it, the choice of browser and Import; saved
 * passwords; how it went) and This browser's Make default; on first login, the
 * offer that asks once.
 */
import { useViewModel } from '../../../hooks/index.ts';
import {
  earlierImports,
  emptyImportText,
  importAmount,
  importBlocked,
  importStatus,
  noSources,
  sourceLabel,
  type ImportViewModel,
} from '../view-models/import-view-model.ts';
import type { ImportRecord, ViewProps } from '../model/models.ts';
import { Button, StatusLine, TimeAgo } from '../../../ui/index.ts';
import { SettingsRow } from './settings-list.tsx';
import { LibraryImportPanel } from './library-import-panel.tsx';

/** The latest import, in two lines; or, before any, what an import is for. */
function Latest({ vm }: ViewProps<ImportViewModel>) {
  const s = useViewModel(vm);
  const latest: ImportRecord | undefined = s.history[0];
  if (!latest)
    return (
      <>
        <p className="row-title" id="import-latest">
          No imports yet
        </p>
        <p className="quiet" id="import-detail">
          {emptyImportText(s.sources)}
        </p>
      </>
    );
  return (
    <>
      <p className="row-title" id="import-latest">{`Imported from ${latest.source}`}</p>
      <p className="quiet" id="import-detail">
        <TimeAgo at={latest.at} />
        {` · ${importAmount(latest)}`}
      </p>
    </>
  );
}

/** The imports before the latest, one line each. */
function Earlier({ vm }: ViewProps<ImportViewModel>) {
  const records = earlierImports(useViewModel(vm));
  return (
    <details className="import-history-disclosure" hidden={!records.length}>
      <summary>
        Previous imports <span>{records.length}</span>
      </summary>
      <ol className="import-history quiet" id="import-history" aria-label="Earlier imports">
        {records.map((r) => (
          <li key={`${r.source}-${r.at}`}>
            {`${r.source} · `}
            <TimeAgo at={r.at} />
            {` · ${importAmount(r)}`}
          </li>
        ))}
      </ol>
    </details>
  );
}

/** Explicit first-login invitation, using the default-first browser picker below. */
function ImportOffer({ vm }: ViewProps<ImportViewModel>) {
  const { offer, sources } = useViewModel(vm);
  if (!offer) return null;
  const source = sources?.find((item) => item.isDefault)?.name || sources?.[0]?.name || 'your browser';
  return (
    <section className="import-offer" aria-label="Import your browser logins">
      <h3>Bring your logins from {source}</h3>
      <p>Your sites stay signed in here, with your bookmarks and history. It takes a few seconds.</p>
      <p className="import-offer-default">
        <Button variant="primary" id="import-make-default" onClick={() => void vm.makeDefault()}>
          Make Oya your default browser
        </Button>
        <span className="quiet">Links you open from other apps come here.</span>
      </p>
    </section>
  );
}

/** The browser to import from, and Import (Import again once there is one); Not now while offering. */
function ImportControls({ vm }: ViewProps<ImportViewModel>) {
  const s = useViewModel(vm);
  return (
    <div className="import-controls" id="import-controls" hidden={noSources(s)}>
      <select
        id="import-source"
        aria-label="Browser to import from"
        value={s.chosen}
        onChange={(e) => vm.choose(e.target.value)}
      >
        {(s.sources ?? []).map((source) => (
          <option key={source.id} value={source.id}>
            {sourceLabel(source)}
          </option>
        ))}
      </select>
      <Button
        variant={s.offer ? 'primary' : 'secondary'}
        id="import-logins"
        disabled={s.running || !!importBlocked(s)}
        onClick={() => void vm.start()}
      >
        {s.history.length ? 'Import again' : 'Import'}
      </Button>
      {s.offer && (
        <Button id="import-not-now" onClick={() => void vm.notNow()}>
          Not now
        </Button>
      )}
    </div>
  );
}

/** How the last import, or what stands in its way, went. */
function ImportStatus({ vm }: ViewProps<ImportViewModel>) {
  const status = importStatus(useViewModel(vm));
  return (
    <StatusLine className="quiet profile-note" id="import-status" data-kind={status.kind}>
      {status.text}
    </StatusLine>
  );
}

/** The Logins section's import rows: from a browser, then saved passwords, with how it went. */
export function ImportRows({ vm }: ViewProps<ImportViewModel>) {
  return (
    <>
      <LibraryImportPanel vm={vm.library} />
      <SettingsRow wide control={<ImportControls vm={vm} />}>
        <Latest vm={vm} />
        <Earlier vm={vm} />
        <ImportStatus vm={vm} />
      </SettingsRow>
      <PasswordsRow vm={vm} />
    </>
  );
}

/** Make Oya the default browser, the first thing to do so links from other apps come here. */
export function DefaultBrowserRow({ vm }: ViewProps<ImportViewModel>) {
  const { isDefault } = useViewModel(vm);
  const button = !isDefault && (
    <Button variant="primary" id="make-default" onClick={() => void vm.makeDefault()}>
      Make default
    </Button>
  );
  return (
    <SettingsRow control={button}>
      <p className="row-title">Default browser</p>
      <p className="quiet" id="default-browser-detail">
        {isDefault
          ? 'Oya is your default browser. Links from other apps open here.'
          : 'Open links from other apps in Oya.'}
      </p>
    </SettingsRow>
  );
}

/** Saved passwords, from an export: the agent signs in with them where a session has expired. */
function PasswordsRow({ vm }: ViewProps<ImportViewModel>) {
  const s = useViewModel(vm);
  const button = (
    <Button
      variant="secondary"
      id="import-passwords"
      disabled={s.running || !s.connected}
      onClick={() => void vm.importPasswords()}
    >
      Import passwords…
    </Button>
  );
  return (
    <SettingsRow control={button}>
      <p className="row-title">Saved passwords</p>
      <p className="quiet">From the Passwords app, Safari, Chrome or a password manager, exported as CSV.</p>
    </SettingsRow>
  );
}

/** The first-login offer: one decision, which browser to bring logins from, and how it went. */
export function ImportGroup({ vm }: ViewProps<ImportViewModel>) {
  return (
    <div className="profile-group" role="group" aria-label="Import your logins">
      <ImportOffer vm={vm} />
      <ImportControls vm={vm} />
      <ImportStatus vm={vm} />
    </div>
  );
}
