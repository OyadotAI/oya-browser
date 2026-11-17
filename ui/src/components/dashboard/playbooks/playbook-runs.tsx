/**
 * A playbook's run history, under its row: each finished run's badge, when it
 * started, how long it took and where, and why a failed one failed. Errors can
 * carry page text, so they are shown as plain text, never as HTML.
 */
'use client';

import { RUN_BADGE_CLASS, RUNS_TEXT } from './constants';
import { runLine } from './format';
import { usePlaybookRuns } from './use-playbook-runs';
import type { PlaybookRunRecord } from './types';

/** Props for the history. */
interface Props {
  /** The caller's API key. */
  apiKey: string;
  /** The playbook whose runs are shown. */
  name: string;
  /** When its newest run ended, as the list last saw it; a change reloads the history. */
  lastEnded?: string;
}

/** One run: a badge that carries its words, its line, and its error. */
function RunItem({ run }: { /** The run. */ run: PlaybookRunRecord }) {
  return (
    <li className="rounded-lg border border-border/60 bg-bg-card/40 px-3 py-2">
      <div className="flex items-center gap-3">
        <span className={`text-[12px] font-medium ${RUN_BADGE_CLASS[run.status] ?? 'text-text-dim'}`}>
          {RUNS_TEXT.status[run.status] ?? run.status}
        </span>
        <span className="num text-[12px] text-text-muted">{runLine(run)}</span>
      </div>
      {run.error && <p className="mt-1 whitespace-pre-wrap text-[12px] text-text-secondary">{run.error}</p>}
    </li>
  );
}

/** Loading, a failed load with Retry, no runs, or the runs. */
function RunsBody({ apiKey, name, lastEnded }: Props) {
  const h = usePlaybookRuns(apiKey, name, lastEnded);
  if (h.failed)
    return (
      <p role="alert" className="flex items-center gap-3 text-[13px] text-red">
        {RUNS_TEXT.loadFailed}
        <button type="button" className="btn-ghost h-7 px-2 text-[12px]" onClick={h.retry}>
          {RUNS_TEXT.retry}
        </button>
      </p>
    );
  if (!h.runs) return <p className="text-[13px] text-text-muted">{RUNS_TEXT.loading}</p>;
  if (!h.runs.length) return <p className="text-[13px] text-text-muted">{RUNS_TEXT.empty}</p>;
  return (
    <ul className="flex flex-col gap-2">
      {h.runs.map((run) => (
        <RunItem key={`${run.startedAt}-${run.browserId}`} run={run} />
      ))}
    </ul>
  );
}

/** The heading and the runs. */
export default function PlaybookRuns({ apiKey, name, lastEnded }: Props) {
  return (
    <section aria-label={RUNS_TEXT.toggle(name)}>
      <h3 className="mb-2 text-[12px] font-medium uppercase tracking-[0.1em] text-text-muted">{RUNS_TEXT.heading}</h3>
      <RunsBody apiKey={apiKey} name={name} lastEnded={lastEnded} />
    </section>
  );
}
