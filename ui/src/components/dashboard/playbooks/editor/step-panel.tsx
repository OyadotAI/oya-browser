/**
 * The panel for one step: its editable fields as inputs, its recorded element
 * read-only, and Delete step, Cancel and Confirm.
 */
'use client';

import { DIRECTIONS, EDITOR_TEXT, STEP_FIELDS, type StepField } from './editor-text';
import { canConfirm, type StepPanel as Panel } from './use-step-panel';
import type { Step } from './definition';

/** Props for the panel. */
interface Props {
  /** The visual view's state, with this step open. */
  p: Panel;
  /** The step. */
  step: Step;
  /** Its index in the steps. */
  index: number;
}

/** One editable field: a select for a scroll's direction, a number for its amount, text otherwise. */
function FieldInput({
  p,
  field,
  first,
}: {
  /** The state. */ p: Panel;
  /** The field. */ field: StepField;
  /** Takes focus on open. */ first: boolean;
}) {
  const common = {
    id: `step-${field}`,
    className: 'field h-8 font-mono',
    autoFocus: first,
    value: p.draft[field] ?? '',
  };
  if (field === 'direction')
    return (
      <select {...common} onChange={(e) => p.setField(field, e.target.value)}>
        {DIRECTIONS.map((d) => (
          <option key={d} value={d}>
            {d}
          </option>
        ))}
      </select>
    );
  const type = field === 'amount' ? 'number' : 'text';
  return <input {...common} type={type} onChange={(e) => p.setField(field, e.target.value)} />;
}

/** The recorded element's handles as a key and value list, never edited here. */
function RecordedElement({ el }: { /** The element. */ el: Step['el'] }) {
  if (!el || typeof el !== 'object') return null;
  return (
    <div>
      <p className="label">{EDITOR_TEXT.recordedElement}</p>
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 font-mono text-[11.5px]">
        {Object.entries(el).map(([k, v]) => (
          <div key={k} className="contents">
            <dt className="text-text-muted">{k}</dt>
            <dd className="truncate text-text" title={String(v)}>
              {String(v)}
            </dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

/** Delete step on the left, Cancel and Confirm on the right. */
function Actions({ p }: { /** The state. */ p: Panel }) {
  return (
    <div className="mt-auto flex items-center gap-2 border-t border-border pt-3">
      <button type="button" className="mr-auto text-[12px] text-red hover:underline" onClick={p.remove}>
        {EDITOR_TEXT.deleteStep}
      </button>
      <button type="button" className="btn-ghost h-7 px-2 text-[12px]" onClick={p.cancel}>
        {EDITOR_TEXT.cancel}
      </button>
      <button
        type="button"
        className="btn-primary h-7 px-3 text-[12px]"
        disabled={!canConfirm(p.draft)}
        onClick={p.confirm}
      >
        {EDITOR_TEXT.confirm}
      </button>
    </div>
  );
}

/** The open step's panel, beside the cards, or over them on a narrow screen. */
export default function StepPanel({ p, step, index }: Props) {
  const fields = Object.keys(p.draft) as StepField[];
  const title = EDITOR_TEXT.stepTitle(index + 1, String(step.action ?? ''));
  return (
    <aside
      aria-label={title}
      className="absolute inset-y-0 right-0 flex w-[320px] flex-col gap-3 overflow-auto border-l border-border bg-bg-elevated p-4 min-[900px]:static"
    >
      <h3 className="font-mono text-[13px] text-text">{title}</h3>
      {fields.map((f, i) => (
        <div key={f}>
          <label className="label" htmlFor={`step-${f}`}>
            {STEP_FIELDS[f]}
          </label>
          <FieldInput p={p} field={f} first={i === 0} />
        </div>
      ))}
      <RecordedElement el={step.el} />
      <Actions p={p} />
    </aside>
  );
}
