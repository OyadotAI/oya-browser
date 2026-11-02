/**
 * The values a playbook routine plays with: one field per variable of the chosen
 * playbook, its saved value as the placeholder. A secret's field is locked and
 * says the playbook's saved value is used, since secrets are never sent back.
 */
'use client';

import FieldHelp from './field-help';
import { EDITOR_TEXT } from './constants';
import type { RoutineForm } from './use-routine-form';

/** One field per variable of the chosen playbook; nothing for a prompt or a playbook without any. */
export default function ValuesField({ form }: { /** The form. */ form: RoutineForm }) {
  const { chosen, fields, set } = form;
  if (!fields.playbook || !chosen?.variables.length) return null;
  const secret = (name: string) => Boolean(chosen.secrets?.includes(name));
  return (
    <fieldset className="mt-3" aria-describedby="rt-values-help">
      <legend className="label">{EDITOR_TEXT.values}</legend>
      {chosen.variables.map((name) => (
        <label key={name} className="mt-2 block text-[12px] text-muted">
          {name}
          <input
            className="field mt-1"
            spellCheck={false}
            disabled={secret(name)}
            placeholder={secret(name) ? EDITOR_TEXT.secret : (chosen.defaults[name] ?? '')}
            value={secret(name) ? '' : (fields.values[name] ?? '')}
            onChange={(e) => set({ values: { ...fields.values, [name]: e.target.value } })}
          />
        </label>
      ))}
      <FieldHelp id="rt-values-help" help={EDITOR_TEXT.valuesHint} />
    </fieldset>
  );
}
