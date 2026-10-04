/**
 * One routine's card: the switch, the name and its last run's pill; the line
 * that says what it is doing; one action (Run now, or Stop for a run on this
 * browser), the history toggle and the menu (Edit, Clear history, Delete);
 * then a note, the delete confirmation and the history when they show.
 */
import { Button, Icon, IconButton } from '../../../ui/index.ts';
import { ROUTINES_TEXT } from '../model/constants.ts';
import { hasFinishedRuns, lastRunLabel, phaseOf, statusLine } from '../model/routine-format.ts';
import { RunBadge, RunList } from './routine-runs.tsx';
import type { RoutinesState, RoutinesViewModel } from '../view-models/routines-view-model.ts';
import type { Phase, Routine } from '../model/types.ts';

/** What a card and its parts are given. */
export interface RoutineCardProps {
  /** The pane. */
  vm: RoutinesViewModel;
  /** The pane's state. */
  state: RoutinesState;
  /** The routine. */
  routine: Routine;
}

/** What the card's main action is given. */
interface ActionProps extends RoutineCardProps {
  /** What the routine is doing. */
  phase: Phase;
}

/** Stop for a run here; otherwise Run now, off with the reason while this app cannot run it. */
function MainAction({ vm, state, routine, phase }: ActionProps) {
  if (phase === 'here')
    return (
      <button type="button" className="routine-action stop" onClick={() => void vm.stop(routine.id)}>
        {ROUTINES_TEXT.stop}
      </button>
    );
  const { busy } = state.snapshot;
  return (
    <button
      type="button"
      className="routine-action"
      disabled={!!busy}
      title={busy}
      onClick={() => void vm.runNow(routine.id)}
    >
      {ROUTINES_TEXT.runNow}
    </button>
  );
}

/** The menu: Edit, Clear history (when there is some) and Delete. */
function RoutineMenu({ vm, routine }: RoutineCardProps) {
  return (
    <div className="routine-menu" role="menu">
      <button type="button" role="menuitem" onClick={() => vm.edit(routine)}>
        {ROUTINES_TEXT.edit}
      </button>
      {hasFinishedRuns(routine) && (
        <button type="button" role="menuitem" onClick={() => void vm.clearHistory(routine.id)}>
          {ROUTINES_TEXT.clear}
        </button>
      )}
      <button type="button" className="danger" role="menuitem" onClick={() => vm.askDelete(routine.id)}>
        {ROUTINES_TEXT.remove}
      </button>
    </div>
  );
}

/** The main action (none while another browser runs it), the history toggle, and the menu. */
function CardFoot({ vm, state, routine, phase }: ActionProps) {
  const name = routine.name ?? '';
  const menuOpen = state.menu === routine.id;
  return (
    <div className="routine-foot">
      {phase !== 'elsewhere' && <MainAction vm={vm} state={state} routine={routine} phase={phase} />}
      <button
        type="button"
        className="routine-history-toggle"
        aria-expanded={state.expanded.includes(routine.id)}
        onClick={() => vm.toggleHistory(routine)}
      >
        {ROUTINES_TEXT.history(routine.runs?.length ?? 0)}
        <span className="routine-chevron">
          <Icon name="chevron" />
        </span>
      </button>
      <IconButton
        type="button"
        className="routine-more"
        aria-label={ROUTINES_TEXT.more(name)}
        aria-haspopup="menu"
        aria-expanded={menuOpen}
        onClick={() => vm.toggleMenu(routine.id)}
        icon="more"
      />
      {menuOpen && <RoutineMenu vm={vm} state={state} routine={routine} />}
    </div>
  );
}

/** "Delete “name” and its history?" with Cancel and Delete. */
function ConfirmDelete({ vm, routine }: RoutineCardProps) {
  return (
    <div className="routine-confirm">
      <span>{ROUTINES_TEXT.confirm(routine.name ?? '')}</span>
      <Button type="button" onClick={() => vm.askDelete(null)}>
        {ROUTINES_TEXT.cancel}
      </Button>
      <button type="button" className="routine-action stop" onClick={() => void vm.remove(routine.id)}>
        {ROUTINES_TEXT.remove}
      </button>
    </div>
  );
}

/** One routine's card. */
export function RoutineCard(props: RoutineCardProps) {
  const { vm, state, routine } = props;
  const { snapshot, now, notes } = state;
  const phase = phaseOf(routine, snapshot.running, now);
  const name = routine.name ?? '';
  return (
    <li className={`routine-card ${phase}`} data-id={routine.id}>
      <div className="routine-top">
        <button
          type="button"
          className="switch"
          role="switch"
          aria-checked={!!routine.enabled}
          aria-label={ROUTINES_TEXT.toggle(name)}
          onClick={() => void vm.setEnabled(routine)}
        />
        <span className="routine-name">{name}</span>
        {routine.target === 'cloud' && (
          <span className="routine-cloud-badge" title={ROUTINES_TEXT.cloudHint}>
            {ROUTINES_TEXT.cloud}
          </span>
        )}
        <RunBadge status={routine.runs?.[0]?.status} label={lastRunLabel(routine, now)} />
      </div>
      <div className="routine-status">{statusLine(routine, phase, snapshot, now)}</div>
      <CardFoot {...props} phase={phase} />
      {Object.hasOwn(notes, routine.id) && <p className="routine-note">{notes[routine.id]}</p>}
      {state.confirming === routine.id && <ConfirmDelete {...props} />}
      {state.expanded.includes(routine.id) && (
        <RunList vm={vm} runs={routine.runs ?? []} browserId={snapshot.browserId} now={now} openRuns={state.openRuns} />
      )}
    </li>
  );
}
