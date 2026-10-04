/**
 * The selected step's editor (#step-editor): a heading, an icon row (on or
 * off, breakpoint, move) with a More menu (duplicate, delete, run to here),
 * the target and value, and the rarely needed fields under Advanced. Every
 * control is off while the draft is locked. Hidden while recording.
 */
import { Button, Icon, IconButton } from '../../../ui/index.ts';
import { STRATEGIES, TARGETED_ACTIONS, VALUE_FIELDS } from '../model/constants.ts';
import { studioMode } from '../model/studio-model.ts';
import { candidateLabel, editorTitle, firstTarget, framesText } from '../model/step-format.ts';
import { StudioField } from './fields.tsx';
import type { StudioPartProps } from './record-pane.tsx';
import type { StudioViewModel } from '../view-models/studio-view-model.ts';
import type { Step } from '../model/types.ts';

/** What every part of the editor is drawn from. */
export interface EditorPartProps {
  /** The studio. */
  vm: StudioViewModel;
  /** The step edited. */
  step: Step;
  /** Every control is off. */
  locked: boolean;
}

/** The step editor, for the selected step. */
export function StepEditorView({ vm, state }: StudioPartProps) {
  const mode = studioMode(state);
  const steps = state.snapshot?.draft.steps ?? [];
  const step = mode.recording ? undefined : steps.find((s) => s.id === state.selected);
  const issue = state.snapshot?.issues.find((item) => item.stepId === step?.id);
  return (
    <section id="step-editor" className="step-editor" hidden={!step} aria-label="Selected step">
      {step && (
        <>
          <div className="editor-heading">
            <h3>{editorTitle(step, steps)}</h3>
            <EditorIcons vm={vm} step={step} locked={mode.locked} runnable={mode.runnable} />
          </div>
          {TARGETED_ACTIONS.includes(step.action) && (
            <EditorTarget key={step.id} vm={vm} step={step} locked={mode.locked} />
          )}
          <ValueField key={'value:' + step.id} vm={vm} step={step} locked={mode.locked} />
          {step.captureIssue && <p className="studio-issue">{step.captureIssue}</p>}
          {issue && <p className="studio-issue">{issue.message}</p>}
          <EditorAdvanced key={'advanced:' + step.id} vm={vm} step={step} locked={mode.locked} />
        </>
      )}
    </section>
  );
}

/** What the icon row is drawn from. */
export interface EditorIconsProps extends EditorPartProps {
  /** The workflow can run, so Run to here is on. */
  runnable: boolean;
}

/** On or off, breakpoint, up, down, and the More menu. */
export function EditorIcons({ vm, step, locked, runnable }: EditorIconsProps) {
  const ed = vm.editor;
  return (
    <div className="editor-icons">
      <IconButton
        type="button"
        icon="power"
        label={step.enabled ? 'Turn step off' : 'Turn step on'}
        on={!step.enabled}
        disabled={locked}
        onClick={() => void ed.toggle(step.id, 'enabled')}
      />
      <IconButton
        type="button"
        icon="record"
        label={step.breakpoint ? 'Remove breakpoint' : 'Pause here'}
        on={step.breakpoint}
        disabled={locked}
        onClick={() => void ed.toggle(step.id, 'breakpoint')}
      />
      <IconButton type="button" icon="up" label="Move up" disabled={locked} onClick={() => void ed.move(step.id, -1)} />
      <IconButton
        type="button"
        icon="down"
        label="Move down"
        disabled={locked}
        onClick={() => void ed.move(step.id, 1)}
      />
      <EditorMore vm={vm} step={step} locked={locked} runnable={runnable} />
    </div>
  );
}

/** The More menu: duplicate, delete, run to here. */
function EditorMore({ vm, step, locked, runnable }: EditorIconsProps) {
  const ed = vm.editor;
  return (
    <details className="editor-more">
      <summary className="icon-button" title="More" aria-label="More">
        <Icon name="more" />
      </summary>
      <div className="editor-menu" onClick={closeMenu}>
        <Button type="button" title="Duplicate" disabled={locked} onClick={() => void ed.duplicate(step.id)}>
          Duplicate
        </Button>
        <Button type="button" title="Delete" disabled={locked} onClick={() => void ed.remove(step.id)}>
          Delete
        </Button>
        <Button type="button" title="Run to here" disabled={locked || !runnable} onClick={() => void ed.runTo(step.id)}>
          Run to here
        </Button>
      </div>
    </details>
  );
}

/** A choice in the More menu closes it, as a menu does. */
const closeMenu = (event: React.MouseEvent<HTMLElement>): void =>
  void event.currentTarget.closest('details')?.removeAttribute('open');

/** The target: strategy and value on one row, and the picker. */
export function EditorTarget({ vm, step, locked }: EditorPartProps) {
  const target = firstTarget(step);
  const set = (key: 'kind' | 'value') => (value: unknown) =>
    void vm.editor.setTarget(step.id, key, String(value ?? ''));
  return (
    <>
      <div className="editor-target">
        <label className="studio-field">
          <span>Find by</span>
          <select
            data-key="strategy"
            value={target.kind}
            disabled={locked}
            onChange={(event) => set('kind')(event.target.value)}
          >
            {Object.entries(STRATEGIES).map(([kind, label]) => (
              <option key={kind} value={kind}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <StudioField label="Target" value={target.value} disabled={locked} onCommit={set('value')} />
      </div>
      <Button type="button" title="Pick on page" disabled={locked} onClick={() => void vm.editor.pick(step.id)}>
        Pick on page
      </Button>
    </>
  );
}

/** The action's value field (URL, text, key, expected result), if it has one. */
export function ValueField({ vm, step, locked }: EditorPartProps) {
  if (!Object.hasOwn(VALUE_FIELDS, step.action)) return null;
  const [key, label] = VALUE_FIELDS[step.action];
  return (
    <StudioField
      label={label}
      value={step[key]}
      disabled={locked}
      onCommit={(value) => void vm.editor.patch(step.id, { [key]: value })}
    />
  );
}

/** The rarely needed fields: ARIA role, frames, timeout and the other recorded targets. */
export function EditorAdvanced({ vm, step, locked }: EditorPartProps) {
  const ed = vm.editor;
  const target = firstTarget(step);
  const others = step.candidates?.slice(1) ?? [];
  return (
    <details className="studio-details">
      <summary>Advanced</summary>
      {target.kind === 'role' && (
        <StudioField
          label="ARIA role"
          value={target.role || 'button'}
          disabled={locked}
          onCommit={(v) => void ed.setTarget(step.id, 'role', String(v ?? ''))}
        />
      )}
      <StudioField
        label="Frames (outer → inner)"
        value={framesText(step.frames)}
        disabled={locked}
        onCommit={(v) => void ed.setFrames(step.id, String(v ?? ''))}
      />
      <StudioField
        label="Timeout (ms)"
        type="number"
        value={step.timeout}
        disabled={locked}
        onCommit={(v) => void ed.setTimeout(step.id, v as number | undefined)}
      />
      {others.length > 0 && (
        <div className="editor-alternatives">
          <span className="eyebrow">Other recorded targets</span>
          {others.map((c, index) => (
            <Button
              type="button"
              title={candidateLabel(c)}
              key={index}
              disabled={locked}
              onClick={() => void ed.chooseTarget(step.id, c)}
            >
              {candidateLabel(c)}
            </Button>
          ))}
        </div>
      )}
    </details>
  );
}
