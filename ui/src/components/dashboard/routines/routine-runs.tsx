/**
 * A routine's run history, under its row: each run's badge and summary, opening
 * to the tools it used and its answer. Answers come from web pages the agent
 * read, so they are shown as plain text, never as HTML.
 */
'use client';

import { BADGE_CLASS, ROUTINES_TEXT, RUNS_TEXT } from './constants';
import { answerOf, badgeLabel, hasFinishedRuns, runSummary } from './format';
import type { Routine, RoutineRun } from './types';

/** Props for the history. */
interface Props {
  /** The routine whose runs are shown. */
  routine: Routine;
  /** The clock times are worded against. */
  now: number;
  /** Clears the finished runs. */
  onClear: () => void;
}

/** A run status as a badge that carries its words, not only a colour. */
export function RunBadge({ status }: { /** The run's status, or none for a routine never run. */ status?: string }) {
  return (
    <span className={`text-[12px] font-medium ${BADGE_CLASS[status ?? ''] ?? 'text-text-dim'}`}>
      {badgeLabel(status)}
    </span>
  );
}

/** One run: its badge and summary, opening to its steps and answer. */
function RunItem({ run, now }: { /** The run. */ run: RoutineRun; /** The clock. */ now: number }) {
  return (
    <li>
      <details className="rounded-lg border border-border/60 bg-bg-card/40 px-3 py-2">
        <summary className="flex cursor-pointer items-center gap-3">
          <RunBadge status={run.status} />
          <span className="num text-[12px] text-text-muted">{runSummary(run, now)}</span>
        </summary>
        <div className="mt-2 flex flex-wrap gap-1">
          {(run.steps ?? []).map((step, i) => (
            <span
              key={`${step}-${i}`}
              className="rounded border border-border px-1.5 py-0.5 font-mono text-[11px] text-text-secondary"
            >
              {step}
            </span>
          ))}
        </div>
        <p className="mt-2 whitespace-pre-wrap text-[13px] text-text-secondary">{answerOf(run)}</p>
      </details>
    </li>
  );
}

/** The runs, newest first, with Clear history when there are finished ones. */
export default function RoutineRuns({ routine, now, onClear }: Props) {
  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <span className="label">{RUNS_TEXT.title}</span>
        {hasFinishedRuns(routine) && (
          <button className="btn-ghost h-7 px-2 text-[12px]" onClick={onClear}>
            {ROUTINES_TEXT.clear}
          </button>
        )}
      </div>
      {routine.runs.length ? (
        <ul className="flex flex-col gap-2">
          {routine.runs.map((run) => (
            <RunItem key={run.id} run={run} now={now} />
          ))}
        </ul>
      ) : (
        <p className="text-[13px] text-text-dim">{RUNS_TEXT.none}</p>
      )}
    </div>
  );
}
