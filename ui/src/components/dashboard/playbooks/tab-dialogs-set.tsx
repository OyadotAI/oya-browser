/**
 * Every overlay the Playbooks tab can open, driven by its state.
 */
'use client';

import type { BrowserRow, Persona } from '../types';
import RecordDialog from './record-dialog';
import RenameDialog from './rename-dialog';
import RunDialog from './run-dialog';
import { CodeDialog, RemoveConfirm } from './tab-dialogs';
import type { PlaybooksTabState } from './use-playbooks-tab';

/** Props for the overlays. */
interface Props {
  /** The tab's state. */
  tab: PlaybooksTabState;
  /** The caller's API key. */
  apiKey: string;
  /** Running browsers. */
  browsers: BrowserRow[];
  /** Profiles. */
  personas: Persona[];
}

/** Code, rename, run, record and delete. */
export default function PlaybookDialogs({ tab, apiKey, browsers, personas }: Props) {
  const { actions: a, dialogs: d, refresh } = tab;
  return (
    <>
      <CodeDialog code={d.code} onClose={() => d.setCode(null)} />
      {a.renaming && (
        <RenameDialog
          current={a.renaming.name}
          busy={a.busy}
          onClose={() => a.setRenaming(null)}
          onRename={a.applyRename}
        />
      )}
      {d.running && (
        <RunDialog
          apiKey={apiKey}
          playbook={d.running}
          browsers={browsers}
          personas={personas}
          onClose={() => d.setRunning(null)}
          onFinished={refresh}
        />
      )}
      {d.recording && (
        <RecordDialog apiKey={apiKey} browsers={browsers} onClose={() => d.setRecording(false)} onSaved={refresh} />
      )}
      <RemoveConfirm
        removing={a.removing}
        busy={a.busy}
        onClose={() => a.setRemoving(null)}
        onConfirm={a.confirmRemove}
      />
    </>
  );
}
