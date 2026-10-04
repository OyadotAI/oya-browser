/**
 * A routine's run history: each run with when it started, how long it took,
 * how it ended, how many steps it took, and whether another Oya browser ran
 * it; opening one shows the steps and the agent's full answer, drawn like an
 * Ask reply (the escaped Markdown of core/markdown.ts, set as HTML here).
 * Also the status badge a card shows for its last run.
 */
import { useMemo } from 'react';
import { RUNS_TEXT } from '../model/constants.ts';
import { answerHtml, badgeLabel, runSummary } from '../model/routine-format.ts';
import type { RoutinesViewModel } from '../view-models/routines-view-model.ts';
import type { RoutineRun } from '../model/types.ts';

/** What a status badge is given. */
export interface RunBadgeProps {
  /** The run's status, or undefined for a routine that never ran. */
  status: string | undefined;
  /** The words, when they say more than the status ("Done · 9:00 AM"). */
  label?: string;
}

/** A status badge. */
export function RunBadge({ status, label }: RunBadgeProps) {
  return <span className={`run-status ${status || 'never'}`}>{label ?? badgeLabel(status)}</span>;
}

/** What the history is given. */
export interface RunListProps {
  /** The pane. */
  vm: RoutinesViewModel;
  /** The runs, newest first. */
  runs: readonly RoutineRun[];
  /** This browser's id, so runs by another say so. */
  browserId: string | undefined;
  /** The time the times are told from. */
  now: number;
  /** Ids of the runs whose details are open. */
  openRuns: readonly string[];
}

/** What one run is given. */
interface RunItemProps extends Omit<RunListProps, 'runs'> {
  /** The run. */
  run: RoutineRun;
}

/** One run: its summary line, opening to its steps and answer. */
function RunItem({ vm, run, browserId, now, openRuns }: RunItemProps) {
  const html = useMemo(() => ({ __html: answerHtml(run) }), [run]);
  return (
    <li className="routine-run">
      <details open={openRuns.includes(run.id)} onToggle={(e) => vm.setRunOpen(run.id, e.currentTarget.open)}>
        <summary>
          <RunBadge status={run.status} />
          <span className="routine-run-when">{runSummary(run, browserId, now)}</span>
        </summary>
        <div className="routine-run-body">
          {!!run.steps?.length && (
            <div className="chat-tools">
              {run.steps.map((name, i) => (
                <span key={i} className="chat-tool-badge">
                  {name}
                </span>
              ))}
            </div>
          )}
          <div className="routine-run-answer chat-msg assistant" dangerouslySetInnerHTML={html} />
        </div>
      </details>
    </li>
  );
}

/** The list of runs, newest first. */
export function RunList({ runs, ...rest }: RunListProps) {
  return (
    <ul className="routine-runs">
      {!runs.length && <li className="routine-runs-empty">{RUNS_TEXT.none}</li>}
      {runs.map((run) => (
        <RunItem key={run.id} run={run} {...rest} />
      ))}
    </ul>
  );
}
