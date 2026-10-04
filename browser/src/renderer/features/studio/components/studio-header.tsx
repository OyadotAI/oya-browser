/**
 * The top of the Record pane: the workflow picker, step count, New and
 * Expand; the storage banner; the record, Test run and Open JSON buttons;
 * and the notes under them (the step limit, what blocks a run).
 */
import { Button, IconButton, StatusLine } from '../../../ui/index.ts';
import { RendererConstants as C } from '../../../core/constants.ts';
import { TOGGLE_LABEL } from '../model/constants.ts';
import {
  expandLabel,
  isDisabled,
  plural,
  stepLimit,
  studioMode,
  toggleTitle,
  withCurrent,
} from '../model/studio-model.ts';
import type { StudioPartProps } from './record-pane.tsx';

/** The picker, count, New and Expand, and the storage banner. */
export function StudioHeader({ vm, state }: StudioPartProps) {
  const s = state.snapshot;
  const expand = expandLabel(state.panelWidth);
  return (
    <>
      <div className="studio-header">
        <DraftPicker vm={vm} state={state} />
        <span id="record-count" className="count-badge" title={`Up to ${C.MAX_STEPS} steps`}>
          {s ? plural(s.draft.steps.length, 'step') : ''}
        </span>
        <IconButton
          id="record-clear"
          label="New workflow"
          data-icon="plus"
          disabled={isDisabled(state, 'record-clear')}
          onClick={() => void vm.actions.newDraft()}
          icon="plus"
        />
        <IconButton
          id="studio-expand"
          label={expand}
          data-icon="expand"
          onClick={() => void vm.actions.expand()}
          icon="expand"
        />
      </div>
      <div className="studio-banner" id="draft-status" role="alert" hidden={!s?.storageError}>
        {s?.storageError || ''}
      </div>
    </>
  );
}

/** What the record controls are drawn from. */
export interface RecordControlsProps extends StudioPartProps {
  /** The record button is refused while watch-only (the control bar's guard). */
  controlBlocked: boolean;
}

/** Record, Test run and Open JSON. */
export function RecordControls({ vm, state, controlBlocked }: RecordControlsProps) {
  const mode = studioMode(state);
  return (
    <div className="record-controls">
      <Button
        variant="primary"
        className={mode.recording ? 'recording' : undefined}
        id="record-toggle"
        title={toggleTitle(navigator.platform)}
        disabled={isDisabled(state, 'record-toggle')}
        data-control-blocked={controlBlocked ? '' : undefined}
        aria-disabled={controlBlocked}
        onClick={() => void vm.actions.toggleRecording()}
      >
        <span className="record-dot" aria-hidden="true" />
        <span id="record-toggle-label">{TOGGLE_LABEL[mode.stage]}</span>
      </Button>
      <Button
        variant="secondary"
        id="record-validate"
        disabled={isDisabled(state, 'record-validate')}
        onClick={() => void vm.actions.validate()}
      >
        Test run
      </Button>
      <Button
        id="record-open"
        title="Open a workflow saved as JSON, or a Chrome Recorder recording"
        disabled={isDisabled(state, 'record-open')}
        onClick={() => void vm.actions.importJson()}
      >
        Open JSON
      </Button>
    </div>
  );
}

/** The step-limit note and what blocks a test run with no step to point at. */
export function StudioNotes({ state }: StudioPartProps) {
  const s = state.snapshot;
  const limit = s ? stepLimit(s.draft.steps.length, studioMode(state).recording) : '';
  const loose = s?.issues.filter((issue) => !issue.stepId) ?? [];
  return (
    <>
      <StatusLine className="studio-status" id="record-limit" hidden={!limit} live>
        {limit}
      </StatusLine>
      <ul className="studio-issues" id="studio-issues" aria-label="What blocks a test run">
        {loose.map((issue, index) => (
          <li key={index}>{issue.message}</li>
        ))}
      </ul>
    </>
  );
}

/** The workflow picker: the stored drafts, an unreadable one shown but not openable. */
export function DraftPicker({ vm, state }: StudioPartProps) {
  const s = state.snapshot;
  const library = s ? withCurrent(s.library, s.draft) : [];
  return (
    <select
      id="draft-library"
      aria-label="Workflow"
      disabled={isDisabled(state, 'draft-library')}
      value={s?.draft.id ?? ''}
      onChange={(event) => void vm.actions.openDraft(event.target.value)}
    >
      {library.map((item) => (
        <option key={item.id} value={item.id} disabled={!!item.error}>
          {item.name + (item.error ? ' · recovery unavailable' : '')}
        </option>
      ))}
    </select>
  );
}
