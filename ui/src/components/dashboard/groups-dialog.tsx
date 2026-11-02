/**
 * The Groups dialog: the key's persona groups, which browser starts name with
 * personaGroup to spread across several profiles. Its table, editor and state
 * live in `personas/`.
 */
'use client';

import { Plus } from 'lucide-react';
import Dialog, { Confirm } from '@/components/ui/dialog';
import GroupEditorDialog from './personas/group-editor';
import GroupList, { CodeLine, EmptyGroups } from './personas/group-list';
import { GROUP_TEXT } from './personas/group-text';
import { useGroups, type GroupsState } from './personas/use-groups';
import type { BrowserRow, Persona } from './types';

/** Props for the dialog. */
interface Props {
  /** Whether it is open. */
  open: boolean;
  /** Closes it. */
  onClose: () => void;
  /** The caller's API key. */
  apiKey: string;
  /** The key's profiles. */
  personas: Persona[];
  /** Running browsers, for the counts. */
  browsers: BrowserRow[];
}

/** The list, or loading, failed or empty. */
function Body({
  g,
  personas,
  browsers,
}: {
  /** The state. */ g: GroupsState;
  /** Profiles. */ personas: Persona[];
  /** Browsers. */ browsers: BrowserRow[];
}) {
  if (g.failed && !g.groups)
    return (
      <div className="flex flex-col items-center gap-3 py-10">
        <p className="text-[13px] text-text-muted">{GROUP_TEXT.failed}</p>
        <button className="btn-ghost h-8" onClick={g.load}>
          {GROUP_TEXT.retry}
        </button>
      </div>
    );
  if (!g.groups)
    return (
      <p role="status" className="py-10 text-center text-[13px] text-text-muted">
        {GROUP_TEXT.loading}
      </p>
    );
  if (!g.groups.length) return <EmptyGroups />;
  return (
    <GroupList
      groups={g.groups}
      personas={personas}
      browsers={browsers}
      onEdit={g.setEditing}
      onDelete={g.setConfirming}
    />
  );
}

/** The groups, New group, the editor and the delete confirmation. */
export default function GroupsDialog({ open, onClose, apiKey, personas, browsers }: Props) {
  const g = useGroups(apiKey, open);
  const saved = () => (g.setEditing(null), g.load());
  const confirming = g.confirming;
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={GROUP_TEXT.title}
      size="lg"
      description={GROUP_TEXT.description}
      footer={
        <button className="btn-ghost" onClick={onClose}>
          {GROUP_TEXT.done}
        </button>
      }
    >
      <div className="space-y-4">
        <div className="flex justify-end">
          <button className="btn-primary h-8" onClick={() => g.setEditing('new')}>
            <Plus className="h-3.5 w-3.5" /> {GROUP_TEXT.newGroup}
          </button>
        </div>
        <Body g={g} personas={personas} browsers={browsers} />
        <CodeLine name={g.groups?.[0]?.name ?? 'my-group'} />
      </div>
      {g.editing && (
        <GroupEditorDialog
          apiKey={apiKey}
          group={g.editing === 'new' ? null : g.editing}
          personas={personas}
          browsers={browsers}
          onClose={() => g.setEditing(null)}
          onSaved={saved}
        />
      )}
      <Confirm
        open={Boolean(confirming)}
        onClose={() => g.setConfirming(null)}
        onConfirm={() => confirming && g.remove(confirming.name).then(() => g.setConfirming(null))}
        title={confirming ? GROUP_TEXT.confirmTitle(confirming.name) : ''}
        body={confirming ? GROUP_TEXT.confirmBody(confirming.name) : ''}
        confirmLabel={GROUP_TEXT.confirmButton}
        danger
        busy={g.deleting}
      />
    </Dialog>
  );
}
