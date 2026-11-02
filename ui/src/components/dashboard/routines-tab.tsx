/**
 * The Routines tab (facade): a project's routines, which run a prompt or a saved
 * playbook on a schedule, on a desktop or in the cloud. The pieces live in
 * ./routines/; the server keeps the routines and works out when each runs next.
 */
'use client';

import { Confirm } from '@/components/ui/dialog';
import RoutineDialog from './routines/routine-dialog';
import RoutineTable from './routines/routine-table';
import TabHeader from './routines/tab-header';
import { ROUTINES_TEXT } from './routines/constants';
import { useRoutinesTab } from './routines/use-routines-tab';
import type { Persona } from './types';

/** Props for the tab. */
interface Props {
  /** The caller's API key. */
  apiKey: string;
  /** The clock times are worded against. */
  now: number;
  /** The project's personas, to pick the one a routine runs as. */
  personas?: Persona[];
}

/** The header, the load error, the table, the editor and the delete confirmation. */
export default function RoutinesTab({ apiKey, now, personas = [] }: Props) {
  const { list, actions, editing, setEditing, saved } = useRoutinesTab(apiKey);
  const confirming = actions.state.confirming;
  return (
    <div className="workspace-list flex h-full min-h-0 min-w-0 flex-col">
      <TabHeader count={(list.routines || []).length} onCreate={() => setEditing('new')} />
      {list.loadError && (
        <div
          role="alert"
          className="mx-4 mb-3 rounded-lg border border-red/30 bg-red/10 px-3 py-2 text-sm text-red lg:mx-6"
        >
          {ROUTINES_TEXT.loadError(list.loadError)}
        </div>
      )}
      <RoutineTable routines={list.routines} now={now} actions={actions} onEdit={setEditing} />
      {editing && (
        <RoutineDialog
          apiKey={apiKey}
          routine={editing === 'new' ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={saved}
          personas={personas}
        />
      )}
      <Confirm
        open={Boolean(confirming)}
        onClose={() => actions.askDelete(null)}
        onConfirm={actions.confirmDelete}
        title={confirming ? ROUTINES_TEXT.confirm(confirming.name) : ''}
        body={ROUTINES_TEXT.confirmBody}
        confirmLabel="Delete"
        danger
        busy={actions.state.deleting}
      />
    </div>
  );
}
