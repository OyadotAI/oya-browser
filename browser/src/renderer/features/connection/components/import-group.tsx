/**
 * "Imported logins" on the account page: the latest import (or what one is
 * for), the ones before it, the choice of browser, Import, and how it went.
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
    <ol className="import-history quiet" id="import-history" aria-label="Earlier imports" hidden={!records.length}>
      {records.map((r) => (
        <li key={`${r.source}-${r.at}`}>
          {`${r.source} · `}
          <TimeAgo at={r.at} />
          {` · ${importAmount(r)}`}
        </li>
      ))}
    </ol>
  );
}

/** The import group. */
export function ImportGroup({ vm }: ViewProps<ImportViewModel>) {
  const s = useViewModel(vm);
  const status = importStatus(s);
  return (
    <div className="profile-group" role="group" aria-labelledby="import-label">
      <h4 className="profile-label" id="import-label">
        Imported logins
      </h4>
      <div className="profile-row">
        <div className="row-text">
          <Latest vm={vm} />
          <Earlier vm={vm} />
        </div>
      </div>
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
          variant="secondary"
          id="import-logins"
          disabled={s.running || !!importBlocked(s)}
          onClick={() => void vm.start()}
        >
          {s.history.length ? 'Import again' : 'Import'}
        </Button>
      </div>
      <StatusLine className="quiet profile-note" id="import-status" data-kind={status.kind}>
        {status.text}
      </StatusLine>
    </div>
  );
}
