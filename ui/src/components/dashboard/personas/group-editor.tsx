/**
 * Make a persona group or change its members: a name (new groups only, since
 * code passes it), and a checklist of the key's profiles without the default
 * one. The server's refusal shows above the buttons.
 */
'use client';

import Dialog from '@/components/ui/dialog';
import { GROUP_NAME_MAX, GROUP_TEXT } from './group-text';
import { eligible } from './group-stats';
import { rowStats } from './model';
import { useGroupEditor, type GroupEditor } from './use-group-editor';
import type { BrowserRow, Persona } from '../types';
import type { PersonaGroup } from './group-api';

/** Props for the editor. */
interface Props {
  /** The caller's API key. */
  apiKey: string;
  /** The group being changed, or null for a new one. */
  group: PersonaGroup | null;
  /** The key's profiles. */
  personas: Persona[];
  /** Running browsers, for each profile's count. */
  browsers: BrowserRow[];
  /** Closes without saving. */
  onClose: () => void;
  /** Runs after a save. */
  onSaved: () => void;
}

/** The name field for a new group, or the fixed name of an existing one. */
function NameField({
  e,
  group,
}: {
  /** The editor. */ e: GroupEditor;
  /** The group, if any. */ group: PersonaGroup | null;
}) {
  if (group)
    return (
      <div className="mb-4">
        <div className="font-mono text-[14px] text-text">{group.name}</div>
        <p className="mt-1 text-[12px] text-text-muted">{GROUP_TEXT.noRename}</p>
      </div>
    );
  return (
    <div className="mb-4">
      <label className="label" htmlFor="group-name">
        {GROUP_TEXT.name}
      </label>
      <input
        id="group-name"
        className="field font-mono"
        autoFocus
        maxLength={GROUP_NAME_MAX}
        placeholder={GROUP_TEXT.namePlaceholder}
        value={e.name}
        onChange={(ev) => e.setName(ev.target.value)}
        aria-describedby="group-name-help"
      />
      <p id="group-name-help" className="mt-1 text-[12px] text-text-muted">
        {GROUP_TEXT.nameHelp}
      </p>
    </div>
  );
}

/** One profile's checkbox, with its running count. */
function ProfileOption({
  p,
  e,
  browsers,
}: {
  /** The profile. */ p: Persona;
  /** The editor. */ e: GroupEditor;
  /** Running browsers. */ browsers: BrowserRow[];
}) {
  const s = rowStats(p, browsers);
  return (
    <label className="flex cursor-pointer items-center gap-3 px-3 py-2 hover:bg-text/[0.035]">
      <input
        type="checkbox"
        className="accent-accent"
        checked={e.picked.includes(p.id)}
        onChange={() => e.toggle(p.id)}
      />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-text">{p.name}</span>
        <span className="block truncate font-mono text-[11px] text-text-dim">{p.id}</span>
      </span>
      <span
        className={`num text-[12px] ${s.at ? 'text-yellow' : s.running ? 'text-accent' : 'text-text-muted'}`}
      >{`${s.running}/${Number.isFinite(s.cap) ? s.cap : '∞'}`}</span>
    </label>
  );
}

/** The checklist, or why there is nothing to pick. */
function Profiles({
  e,
  personas,
  browsers,
}: {
  /** The editor. */ e: GroupEditor;
  /** Pickable profiles. */ personas: Persona[];
  /** Running browsers. */ browsers: BrowserRow[];
}) {
  if (!personas.length) return <p className="text-[13px] text-text-muted">{GROUP_TEXT.noEligible}</p>;
  return (
    <fieldset>
      <legend className="label">
        {GROUP_TEXT.profiles}{' '}
        <span className="normal-case tracking-normal text-text-dim">· {e.picked.length} selected</span>
      </legend>
      <div className="max-h-72 divide-y divide-border/60 overflow-y-auto rounded-lg border border-border">
        {personas.map((p) => (
          <ProfileOption key={p.id} p={p} e={e} browsers={browsers} />
        ))}
      </div>
      <p className="mt-2 text-[12px] text-text-muted">{e.picked.length ? GROUP_TEXT.howPicked : GROUP_TEXT.pickOne}</p>
    </fieldset>
  );
}

/** The editor in a dialog. */
export default function GroupEditorDialog({ apiKey, group, personas, browsers, onClose, onSaved }: Props) {
  const e = useGroupEditor(apiKey, group, onSaved);
  const pickable = eligible(personas);
  const footer = (
    <>
      <button type="button" className="btn-ghost" onClick={onClose} disabled={e.saving}>
        {GROUP_TEXT.cancel}
      </button>
      {pickable.length > 0 && (
        <button type="submit" form="group-form" className="btn-primary" disabled={!e.canSave || e.saving}>
          {e.saving ? GROUP_TEXT.saving : group ? GROUP_TEXT.save : GROUP_TEXT.create}
        </button>
      )}
    </>
  );
  return (
    <Dialog
      open
      onClose={onClose}
      size="md"
      title={group ? GROUP_TEXT.editTitle(group.name) : GROUP_TEXT.newTitle}
      footer={footer}
    >
      <form id="group-form" noValidate onSubmit={(ev) => (ev.preventDefault(), e.canSave && !e.saving && e.save())}>
        <NameField e={e} group={group} />
        <Profiles e={e} personas={pickable} browsers={browsers} />
        {e.error && (
          <p role="alert" className="mt-3 text-[13px] text-red">
            {e.error}
          </p>
        )}
      </form>
    </Dialog>
  );
}
