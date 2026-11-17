/**
 * The definition drawn as its steps: Start, one card per step in replay order,
 * End, and the panel for the step a person opens. Playbooks replay in a line, so
 * this is a column, not a free canvas. It reads the parsed definition and writes
 * only on Confirm or Delete, so switching views never changes the text.
 */
'use client';

import type { KeyboardEvent } from 'react';
import { EDITOR_TEXT } from './editor-text';
import { stepSummary, stepsOf, type Step } from './definition';
import StepPanel from './step-panel';
import { END_ID, cardId, useStepPanel, type StepPanel as Panel } from './use-step-panel';
import type { Definition } from './editor-api';

/** Props for the view. */
interface Props {
  /** The parsed definition. */
  definition: Definition;
  /** Replaces the editor text. */
  onChange: (text: string) => void;
}

/** The Start or End marker, focusable by script so focus has somewhere to land. */
function Pill({ label, id }: { /** Its words. */ label: string; /** Its id. */ id?: string }) {
  return (
    <li className="flex justify-center">
      <span
        id={id}
        tabIndex={-1}
        className="rounded-full border border-border bg-bg-card px-3 py-1 text-[11px] uppercase tracking-[0.1em] text-text-muted outline-none focus-visible:ring-1 focus-visible:ring-accent"
      >
        {label}
      </span>
    </li>
  );
}

/** One step as a card: its number and action, and a line on what it does. */
function StepCard({
  step,
  i,
  open,
  onOpen,
}: {
  /** The step. */ step: Step;
  /** Its index. */ i: number;
  /** Its panel is open. */ open: boolean;
  /** Opens its panel. */ onOpen: () => void;
}) {
  const summary = stepSummary(step);
  return (
    <li className="flex flex-col">
      <div className="mx-auto h-3 w-px bg-border" aria-hidden />
      <button
        id={cardId(i)}
        type="button"
        aria-expanded={open}
        onClick={onOpen}
        title={summary}
        className={`flex w-full items-baseline gap-3 rounded border bg-bg-card px-3 py-2 text-left text-[13px] hover:border-accent/60 ${open ? 'border-accent' : 'border-border'}`}
      >
        <span className="shrink-0 font-mono text-text">{`${i + 1} · ${String(step.action ?? '')}`}</span>
        <span className="min-w-0 truncate text-text-muted">{summary}</span>
      </button>
      <div className="mx-auto h-3 w-px bg-border" aria-hidden />
    </li>
  );
}

/**
 * Escape anywhere in the view cancels the open step and the workbench does not
 * see it, which would otherwise offer to close the whole editor. Caught here, not
 * on the panel, because a click step has no field to take focus into it.
 */
const onEscape = (p: Panel) => (e: KeyboardEvent) => {
  if (e.key !== 'Escape' || p.open === null) return;
  e.preventDefault();
  p.cancel();
};

/** Start, the step cards, End, and the open step's panel. */
export default function StepFlow({ definition, onChange }: Props) {
  const p = useStepPanel(definition, onChange);
  const steps = stepsOf(definition);
  return (
    <div className="relative flex h-full min-h-0" onKeyDown={onEscape(p)}>
      <div className="min-w-0 flex-1 overflow-auto p-6">
        <ol aria-label={EDITOR_TEXT.stepsLabel} className="mx-auto flex max-w-[560px] flex-col">
          <Pill label={EDITOR_TEXT.start} />
          {steps.map((s, i) => (
            <StepCard key={i} step={s} i={i} open={p.open === i} onOpen={() => p.openStep(i)} />
          ))}
          {!steps.length && <li className="py-4 text-center text-[13px] text-text-muted">{EDITOR_TEXT.emptySteps}</li>}
          <Pill label={EDITOR_TEXT.end} id={END_ID} />
        </ol>
      </div>
      {p.open !== null && steps[p.open] && <StepPanel key={p.open} p={p} step={steps[p.open]} index={p.open} />}
    </div>
  );
}
