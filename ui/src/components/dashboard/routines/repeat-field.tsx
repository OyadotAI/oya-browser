/**
 * When a routine runs: every N minutes or hours, or daily at a time read in the
 * person's own time zone.
 */
'use client';

import FieldHelp from './field-help';
import { EDITOR_TEXT, EVERY_MAX } from './constants';
import { ownZone, type RoutineForm } from './use-routine-form';

/** Props for the repeat fields. */
interface Props {
  /** The form. */
  form: RoutineForm;
}

/** One of the two segmented choices. */
function KindButton({
  form,
  kind,
  label,
}: Props & { /** The kind it picks. */ kind: 'every' | 'daily'; /** Its words. */ label: string }) {
  const on = form.fields.kind === kind;
  return (
    <button
      type="button"
      role="radio"
      aria-checked={on}
      className={`btn-ghost h-8 px-3 text-[12px] ${on ? 'text-accent' : ''}`}
      onClick={() => form.set({ kind })}
    >
      {label}
    </button>
  );
}

/** The count and unit for every N. */
function EveryInputs({ form }: Props) {
  const { fields, set, errors } = form;
  return (
    <>
      <input
        type="number"
        min={1}
        max={EVERY_MAX}
        className="field w-24"
        aria-label={EDITOR_TEXT.howOften}
        value={fields.n}
        onChange={(e) => set({ n: e.target.value })}
        aria-describedby="rt-repeat-help"
        aria-invalid={Boolean(errors.every)}
      />
      <select
        className="field w-32"
        aria-label={EDITOR_TEXT.unit}
        value={fields.unit}
        onChange={(e) => set({ unit: e.target.value })}
      >
        <option value="hours">hours</option>
        <option value="minutes">minutes</option>
      </select>
    </>
  );
}

/** The kind choice, then its inputs. */
export default function RepeatField({ form }: Props) {
  const { fields, set, errors } = form;
  const daily = fields.kind === 'daily';
  return (
    <div className="mt-3">
      <span id="rt-repeat" className="label">
        {EDITOR_TEXT.repeat}
      </span>
      <div className="flex flex-wrap items-center gap-2">
        <div role="radiogroup" aria-labelledby="rt-repeat" className="flex gap-1">
          <KindButton form={form} kind="every" label={EDITOR_TEXT.every} />
          <KindButton form={form} kind="daily" label={EDITOR_TEXT.dailyAt} />
        </div>
        {daily ? (
          <input
            type="time"
            className="field w-32"
            aria-label={EDITOR_TEXT.timeOfDay}
            value={fields.at}
            onChange={(e) => set({ at: e.target.value })}
          />
        ) : (
          <EveryInputs form={form} />
        )}
      </div>
      <FieldHelp
        id="rt-repeat-help"
        error={daily ? undefined : errors.every}
        help={daily ? EDITOR_TEXT.inZone(ownZone()) : undefined}
      />
    </div>
  );
}
