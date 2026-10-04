/**
 * The Steps tab: Undo, Redo and Add step; the step list (one row per step
 * with its number or breakpoint, action, target and any issue; a click
 * selects it); the selected step's editor; and the variables.
 */
import { useLayoutEffect, useRef } from 'react';
import { ACTION_NAMES, ADDABLE_ACTIONS } from '../model/constants.ts';
import { actionName, isDisabled } from '../model/studio-model.ts';
import { stepClass, stepNumber, stepValue } from '../model/step-format.ts';
import type { StudioPartProps } from './record-pane.tsx';
import type { Issue, Step, Variables } from '../model/types.ts';
import { StepEditorView } from './step-editor-view.tsx';
import { VariablesPanel } from './variables-panel.tsx';
import { Button } from '../../../ui/index.ts';

/** The Steps tab. */
export function StepsTab({ vm, state }: StudioPartProps) {
  return (
    <>
      <StepsToolbar vm={vm} state={state} />
      <StepList vm={vm} state={state} />
      <StepEditorView vm={vm} state={state} />
      <VariablesPanel vm={vm} state={state} />
    </>
  );
}

/** Undo, Redo and Add step. */
export function StepsToolbar({ vm, state }: StudioPartProps) {
  return (
    <div className="studio-toolbar">
      <Button id="step-undo" disabled={isDisabled(state, 'step-undo')} onClick={() => void vm.actions.travel('undo')}>
        Undo
      </Button>
      <Button id="step-redo" disabled={isDisabled(state, 'step-redo')} onClick={() => void vm.actions.travel('redo')}>
        Redo
      </Button>
      <select
        id="step-add"
        aria-label="Add step"
        value=""
        disabled={isDisabled(state, 'step-add')}
        onChange={(event) => void vm.actions.addStep(event.target.value)}
      >
        <option value="">+ Add step</option>
        {ADDABLE_ACTIONS.map((action) => (
          <option key={action} value={action}>
            {ACTION_NAMES[action]}
          </option>
        ))}
      </select>
    </div>
  );
}

/** The step list; while recording it follows each new step to the bottom. */
export function StepList({ vm, state }: StudioPartProps) {
  const list = useRef<HTMLDivElement>(null);
  const steps = state.snapshot?.draft.steps ?? [];
  useLayoutEffect(() => {
    if (!state.followSteps || !list.current) return;
    list.current.scrollTop = list.current.scrollHeight;
    vm.followed();
  }, [vm, state.followSteps]);
  const variables = state.snapshot?.draft.variables ?? {};
  const issues = state.snapshot?.issues ?? [];
  const row = (step: Step, index: number) => (
    <StepRow
      key={step.id}
      step={step}
      index={index}
      selected={state.selected === step.id}
      variables={variables}
      issue={issues.find((item) => item.stepId === step.id)}
      onSelect={() => vm.select(step.id)}
    />
  );
  return (
    <div className="rec-steps" id="record-steps" aria-label="Recorded steps" ref={list}>
      {steps.length ? steps.map(row) : <EmptySteps />}
    </div>
  );
}

/** What an empty draft shows. */
function EmptySteps() {
  return (
    <div className="record-empty">
      <h3>No steps yet</h3>
      <p>Record what you do in the page, or add steps by hand.</p>
    </div>
  );
}

/** What a step row shows. */
export interface StepRowProps {
  /** The step. */
  step: Step;
  /** Its place in the list. */
  index: number;
  /** Whether it is selected. */
  selected: boolean;
  /** The draft's variables, to show a secret as one. */
  variables: Variables;
  /** What about it blocks a test run, if anything. */
  issue: Issue | undefined;
  /** Selects it. */
  onSelect: () => void;
}

/** One step's row; a step that blocks a test run says why, on hover and to a screen reader. */
export function StepRow({ step, index, selected, variables, issue, onSelect }: StepRowProps) {
  return (
    <button type="button" className={stepClass(step, selected)} aria-pressed={selected} onClick={onSelect}>
      <span className="step-number">{stepNumber(step, index)}</span>
      <span className="step-copy">
        <strong>{actionName(step.action)}</strong>
        <span className="step-value">{stepValue(step, variables)}</span>
      </span>
      {issue && (
        <span className="step-issue" title={issue.message} aria-label={'Blocks a test run: ' + issue.message}>
          !
        </span>
      )}
    </button>
  );
}
