/**
 * Make or change a routine: its name, what it does (and a playbook's values),
 * when it repeats, the profile and place it runs, and whether it is on. The server's refusal shows above the buttons.
 */
'use client';

import Dialog from '@/components/ui/dialog';
import FieldHelp from './field-help';
import RepeatField from './repeat-field';
import TaskField from './task-field';
import ValuesField from './values-field';
import ProfileField from './profile-field';
import { EDITOR_LIMITS, EDITOR_TEXT } from './constants';
import { useRoutineForm, type RoutineForm } from './use-routine-form';
import type { Routine } from './types';
import type { Persona } from '../types';

/** Props for the dialog. */
interface Props {
  /** The caller's API key. */
  apiKey: string;
  /** The routine being edited, or null for a new one. */
  routine: Routine | null;
  /** Closes without saving. */
  onClose: () => void;
  /** Receives the saved routine. */
  onSaved: (r: Routine) => void;
  /** The project's personas, to pick the one it runs as. */
  personas: Persona[];
}

/** A labelled checkbox with its help. */
function Check({
  id,
  label,
  help,
  checked,
  onChange,
}: {
  /** Its id. */ id: string;
  /** Its label. */ label: string;
  /** Its help. */ help?: string;
  /** Ticked. */ checked: boolean;
  /** Flips it. */ onChange: (v: boolean) => void;
}) {
  return (
    <div className="mt-3">
      <label className="flex items-center gap-2 text-[13px] text-text" htmlFor={id}>
        <input
          id={id}
          type="checkbox"
          className="accent-accent"
          checked={checked}
          onChange={(e) => onChange(e.target.checked)}
          aria-describedby={help ? `${id}-help` : undefined}
        />
        {label}
      </label>
      <FieldHelp id={`${id}-help`} help={help} />
    </div>
  );
}

/** The name field. */
function NameField({ form }: { /** The form. */ form: RoutineForm }) {
  return (
    <div className="mb-3">
      <label className="label" htmlFor="rt-name">
        {EDITOR_TEXT.name}
      </label>
      <input
        id="rt-name"
        className="field"
        autoFocus
        maxLength={EDITOR_LIMITS.nameChars}
        placeholder={EDITOR_TEXT.namePlaceholder}
        value={form.fields.name}
        onChange={(e) => form.set({ name: e.target.value })}
        aria-describedby="rt-name-help"
        aria-invalid={Boolean(form.errors.name)}
      />
      <FieldHelp id="rt-name-help" error={form.errors.name} />
    </div>
  );
}

/** Cancel and Save. */
function Footer({
  form,
  editing,
  onClose,
}: {
  /** The form. */ form: RoutineForm;
  /** Editing an existing routine. */ editing: boolean;
  /** Closes. */ onClose: () => void;
}) {
  const label = editing ? EDITOR_TEXT.update : EDITOR_TEXT.create;
  return (
    <>
      <button type="button" className="btn-ghost" onClick={onClose} disabled={form.saving}>
        {EDITOR_TEXT.cancel}
      </button>
      <button type="submit" form="routine-form" className="btn-primary" disabled={form.saving}>
        {form.saving ? EDITOR_TEXT.saving : label}
      </button>
    </>
  );
}

/** The form in a dialog. */
export default function RoutineDialog({ apiKey, routine, onClose, onSaved, personas }: Props) {
  const form = useRoutineForm(apiKey, routine, onSaved);
  const { fields, set } = form;
  return (
    <Dialog
      open
      onClose={onClose}
      title={routine ? EDITOR_TEXT.editTitle : EDITOR_TEXT.newTitle}
      footer={<Footer form={form} editing={Boolean(routine)} onClose={onClose} />}
    >
      <form id="routine-form" noValidate onSubmit={(e) => (e.preventDefault(), form.save())}>
        <NameField form={form} />
        <TaskField form={form} />
        <ValuesField form={form} />
        <RepeatField form={form} />
        <ProfileField form={form} personas={personas} />
        <Check
          id="rt-cloud"
          label={EDITOR_TEXT.cloud}
          help={fields.cloud ? EDITOR_TEXT.cloudHint : EDITOR_TEXT.desktopHint}
          checked={fields.cloud}
          onChange={(cloud) => set({ cloud })}
        />
        <Check
          id="rt-enabled"
          label={EDITOR_TEXT.enabled}
          checked={fields.enabled}
          onChange={(enabled) => set({ enabled })}
        />
        {form.saveError && (
          <p role="alert" className="mt-3 text-[13px] text-red">
            {form.saveError}
          </p>
        )}
      </form>
    </Dialog>
  );
}
