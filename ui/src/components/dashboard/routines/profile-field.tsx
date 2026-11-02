/**
 * The profile a routine runs as: the project's default, or one of its personas.
 * A cloud run starts on it; a desktop runs the routine only while it is on it.
 */
'use client';

import FieldHelp from './field-help';
import { EDITOR_TEXT } from './constants';
import type { RoutineForm } from './use-routine-form';
import type { Persona } from '../types';

/** Props for the profile field. */
interface Props {
  /** The form. */
  form: RoutineForm;
  /** The project's personas. */
  personas: Persona[];
}

/** The Profile choice, the default first; a profile no longer listed still shows by its id. */
export default function ProfileField({ form, personas }: Props) {
  const { fields, set } = form;
  const named = personas.filter((p) => !p.isDefault).map((p) => ({ id: p.id, name: p.name }));
  const listed = named.some((p) => p.id === fields.persona);
  const options = fields.persona && !listed ? [{ id: fields.persona, name: fields.persona }, ...named] : named;
  const help = fields.persona ? (fields.cloud ? EDITOR_TEXT.profileCloudHint : EDITOR_TEXT.profileHint) : undefined;
  return (
    <div className="mt-3">
      <label className="label" htmlFor="rt-persona">
        {EDITOR_TEXT.profile}
      </label>
      <select
        id="rt-persona"
        className="field"
        value={fields.persona}
        onChange={(e) => set({ persona: e.target.value })}
        aria-describedby="rt-persona-help"
      >
        <option value="">{EDITOR_TEXT.defaultProfile}</option>
        {options.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
          </option>
        ))}
      </select>
      <FieldHelp id="rt-persona-help" help={help} />
    </div>
  );
}
