/**
 * One routine in the table: what it does, its schedule and where it runs, its
 * switch, its last and next run, and its buttons; its history opens under it.
 */
'use client';

import { ChevronDown, Pencil, Trash2 } from 'lucide-react';
import Switch from './switch';
import { RunBadge } from './routine-runs';
import { ROUTINES_TEXT } from './constants';
import { lastRunLabel, nextLine, phaseOf, scheduleOf as scheduleText } from './format';
import type { RoutineActions } from './use-routine-actions';
import type { Routine } from './types';

/** Props for a row. */
export interface RowProps {
  /** The routine shown. */
  routine: Routine;
  /** The clock times are worded against. */
  now: number;
  /** What the buttons do, and what is in flight. */
  actions: RoutineActions;
  /** Whether its history is open. */
  open: boolean;
  /** Opens or closes its history. */
  onHistory: () => void;
  /** Opens the editor on it. */
  onEdit: () => void;
}

/** The name, and the playbook it plays or the first line of its prompt. */
function TaskCell({ routine }: { /** The routine. */ routine: Routine }) {
  const firstLine = routine.prompt.split('\n')[0];
  return (
    <td className="px-4 py-3">
      <div className="truncate font-medium text-text" title={routine.name}>
        {routine.name}
      </div>
      <div className="truncate text-[11px] text-text-dim" title={routine.playbook || routine.prompt}>
        {routine.playbook ? <span className="font-mono">{routine.playbook}</span> : firstLine}
      </div>
    </td>
  );
}

/** The last run's badge, with its time. */
function LastRunCell({ routine, now }: Pick<RowProps, 'routine' | 'now'>) {
  const run = routine.runs[0];
  const label = lastRunLabel(routine, now);
  return (
    <td className="px-3 py-3" title={label}>
      <RunBadge status={run?.status} />
      {run && run.status !== 'running' && (
        <span className="num text-[12px] text-text-muted">{label.slice(label.indexOf(' · '))}</span>
      )}
    </td>
  );
}

/** When it runs next, and the row's note about the last Run now. */
function NextCell({
  routine,
  now,
  note,
}: Pick<RowProps, 'routine' | 'now'> & { /** The row's note. */ note?: string }) {
  const off = phaseOf(routine, now) === 'off';
  return (
    <td className="px-3 py-3">
      <span className={`num text-[12px] ${off ? 'text-text-dim' : 'text-text-secondary'}`}>
        {nextLine(routine, now)}
      </span>
      <p role="status" aria-live="polite" className="text-[12px] text-yellow">
        {note ?? ''}
      </p>
    </td>
  );
}

/** Run now for a cloud routine; a desktop one keeps the slot so the columns line up. */
function RunNowButton({ routine, actions, now }: Pick<RowProps, 'routine' | 'actions' | 'now'>) {
  if (routine.target !== 'cloud') return <span className="inline-block w-[68px]" aria-hidden />;
  const starting = Boolean(actions.state.starting[routine.id]);
  const running = phaseOf(routine, now) === 'running';
  return (
    <button
      className="btn-ghost h-7 px-2 text-[12px]"
      disabled={starting || running}
      aria-busy={starting}
      title={running ? ROUTINES_TEXT.alreadyRunning : undefined}
      onClick={() => actions.runNow(routine)}
    >
      {starting ? ROUTINES_TEXT.starting : ROUTINES_TEXT.runNow}
    </button>
  );
}

/** Run now, History, Edit and Delete. */
function ActionsCell({ routine, now, actions, open, onHistory, onEdit }: RowProps) {
  return (
    <td className="px-2 py-3">
      <div className="flex items-center justify-end gap-1">
        <RunNowButton routine={routine} actions={actions} now={now} />
        <button
          className="btn-ghost h-7 px-2 text-[12px]"
          aria-expanded={open}
          aria-controls={`runs-${routine.id}`}
          onClick={onHistory}
        >
          {ROUTINES_TEXT.history(routine.runs.length)}
          <ChevronDown className={`h-3.5 w-3.5 transition-transform ${open ? 'rotate-180' : ''}`} />
        </button>
        <button className="btn-icon" aria-label={ROUTINES_TEXT.edit(routine.name)} title="Edit" onClick={onEdit}>
          <Pencil className="h-4 w-4" />
        </button>
        <button
          className="btn-icon"
          aria-label={ROUTINES_TEXT.remove(routine.name)}
          title="Delete"
          onClick={() => actions.askDelete(routine)}
        >
          <Trash2 className="h-4 w-4" />
        </button>
      </div>
    </td>
  );
}

/** The row's cells. */
export default function RoutineRow(props: RowProps) {
  const { routine, now, actions } = props;
  const pending = actions.state.pending[routine.id];
  return (
    <tr className="border-b border-border/60">
      <TaskCell routine={routine} />
      <td className="px-3 py-3 text-text-secondary">{scheduleText(routine)}</td>
      <td className="px-3 py-3 text-text-secondary">
        {routine.target === 'cloud' ? ROUTINES_TEXT.cloud : ROUTINES_TEXT.desktop}
      </td>
      <td className="px-3 py-3">
        <Switch
          checked={pending ?? routine.enabled}
          disabled={pending !== undefined}
          label={ROUTINES_TEXT.toggle(routine.name)}
          onChange={() => actions.toggle(routine)}
        />
      </td>
      <LastRunCell routine={routine} now={now} />
      <NextCell routine={routine} now={now} note={actions.state.notes[routine.id]} />
      <ActionsCell {...props} />
    </tr>
  );
}
