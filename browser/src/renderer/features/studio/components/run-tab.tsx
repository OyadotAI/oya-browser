/**
 * The Run tab: the run's title and summary, the controls that work in its
 * status, the replay speed, an input per variable, the timeline (a step's
 * event opens that step), repairs to review, and previous runs.
 */
import type { KeyboardEvent } from 'react';
import { RUN_CONTROLS, RUN_SPEEDS } from '../model/constants.ts';
import { isActive, isDisabled } from '../model/studio-model.ts';
import {
  canApply,
  controlEnabled,
  eventDuration,
  eventNumber,
  eventText,
  historyLabel,
  repairNote,
  runSummary,
  runTitle,
  shownEvents,
} from '../model/run-format.ts';
import { Button } from '../../../ui/index.ts';
import type { StudioPartProps } from './record-pane.tsx';

/** The Run tab. */
export function RunTab({ vm, state }: StudioPartProps) {
  const run = state.snapshot?.run;
  return (
    <>
      <div className="run-heading">
        <h3 id="run-title">{runTitle(run)}</h3>
        <p id="run-summary">{runSummary(run)}</p>
      </div>
      <div className="run-controls" id="run-controls" hidden={!isActive(run)}>
        {RUN_CONTROLS.map(({ command, label }) => (
          <Button
            key={command}
            variant={command === 'stop' ? 'text' : 'secondary'}
            data-run={command}
            disabled={!controlEnabled(run, command)}
            onClick={() => void vm.run.control(command)}
          >
            {label}
          </Button>
        ))}
      </div>
      <label className="studio-field run-speed">
        <span>Replay speed</span>
        <select
          id="run-speed"
          value={state.runSpeed}
          onChange={(event) => vm.run.setSpeed(Number(event.target.value) || 0)}
        >
          {RUN_SPEEDS.map(({ value, label }) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </select>
      </label>
      <RunInputs vm={vm} state={state} />
      <RunEvents vm={vm} state={state} />
      <RunRepairs vm={vm} state={state} />
      <RunHistory vm={vm} state={state} />
    </>
  );
}

/** One input per variable the steps use; a secret one is a password field. */
export function RunInputs({ vm, state }: StudioPartProps) {
  const variables = state.snapshot?.draft.variables ?? {};
  return (
    <div id="run-inputs">
      {Object.entries(state.runInputs).map(([name, value]) => (
        <label key={name} className="studio-field">
          <span>{name + (variables[name]?.secret ? ' · secret' : '')}</span>
          <input
            type={variables[name]?.secret ? 'password' : 'text'}
            data-variable={name}
            autoComplete="off"
            value={value}
            onChange={(event) => vm.run.setInput(name, event.target.value)}
          />
        </label>
      ))}
    </div>
  );
}

/** The timeline: each step's latest event; a step's event opens that step by click or Enter. */
export function RunEvents({ vm, state }: StudioPartProps) {
  const steps = state.snapshot?.draft.steps ?? [];
  const linked = (stepId: string | undefined) => !!stepId && steps.some((s) => s.id === stepId);
  const open = (stepId: string) => () => vm.run.openStep(stepId);
  return (
    <div id="run-events" aria-label="Test run timeline">
      {shownEvents(state.snapshot?.run).map((event, index) => (
        <div
          key={(event.stepId ?? '') + ':' + index}
          className={'run-event ' + (event.status || '')}
          tabIndex={linked(event.stepId) ? 0 : undefined}
          onClick={linked(event.stepId) ? open(event.stepId!) : undefined}
          onKeyDown={
            linked(event.stepId) ? (e: KeyboardEvent) => e.key === 'Enter' && open(event.stepId!)() : undefined
          }
        >
          <span className="step-number">{eventNumber(event, steps)}</span>
          <span>{eventText(event, steps)}</span>
          <small>{eventDuration(event)}</small>
        </div>
      ))}
    </div>
  );
}

/** The repairs the run found: apply one to this workflow, or review it as a copy. */
export function RunRepairs({ vm, state }: StudioPartProps) {
  const s = state.snapshot;
  return (
    <div id="run-repairs">
      {(s?.run?.repairs ?? []).map((repair) => (
        <div key={repair.draftId} className="repair-review">
          <strong>Repair found</strong>
          <p>{repairNote(repair)}</p>
          <Button
            type="button"
            title="Apply to this workflow"
            disabled={!s || !canApply(s, repair)}
            onClick={() => void vm.run.applyRepair(repair)}
          >
            Apply to this workflow
          </Button>
          <Button type="button" title="Review as a copy" onClick={() => void vm.run.reviewRepair(repair)}>
            Review as a copy
          </Button>
        </div>
      ))}
    </div>
  );
}

/** The previous runs, with this run selected. */
export function RunHistory({ vm, state }: StudioPartProps) {
  const s = state.snapshot;
  return (
    <div className="studio-library">
      <select
        id="run-history"
        aria-label="Previous test runs"
        value={s?.run?.id ?? ''}
        disabled={isDisabled(state, 'run-history')}
        onChange={(event) => void vm.run.openRun(event.target.value)}
      >
        <option value="">Previous runs</option>
        {(s?.runHistory ?? []).map((item) => (
          <option key={item.id} value={item.id}>
            {historyLabel(item)}
          </option>
        ))}
      </select>
    </div>
  );
}
