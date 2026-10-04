/**
 * The routine editor's card (`#routine-form`): name, prompt and schedule, with
 * the refusal the server gave. Opening it scrolls it into view and focuses the
 * name field.
 */
import { useEffect } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { EDITOR_LIMITS, EDITOR_TEXT } from '../model/constants.ts';
import type { RoutineEditorViewModel, ScheduleKind } from '../view-models/routine-editor-view-model.ts';
import { Button } from '../../../ui/index.ts';

/** What the editor is given. */
export interface RoutineEditorProps {
  /** The editor. */
  editor: RoutineEditorViewModel;
}

/** The schedule kinds, as the radio buttons read. */
const KINDS: readonly [ScheduleKind, string][] = [
  ['every', 'Every'],
  ['daily', 'Daily at'],
];

/** Every or Daily at, and the fields of the one picked. */
function ScheduleFields({ editor }: RoutineEditorProps) {
  const { kind, n, unit, at } = useViewModel(editor);
  return (
    <div className="routine-schedule">
      <div className="routine-kind" id="routine-kind" role="radiogroup" aria-labelledby="routine-repeat-label">
        {KINDS.map(([value, label]) => (
          <button
            key={value}
            type="button"
            role="radio"
            data-kind={value}
            aria-checked={kind === value}
            onClick={() => editor.setKind(value)}
          >
            {label}
          </button>
        ))}
      </div>
      <input
        id="routine-n"
        type="number"
        min="1"
        max="999"
        aria-label="How often"
        hidden={kind === 'daily'}
        value={n}
        onChange={(e) => editor.setField('n', e.target.value)}
      />
      <select
        id="routine-unit"
        aria-label="Unit"
        hidden={kind === 'daily'}
        value={unit}
        onChange={(e) => editor.setField('unit', e.target.value)}
      >
        <option value="hours">hours</option>
        <option value="minutes">minutes</option>
      </select>
      <input
        id="routine-at"
        type="time"
        aria-label="Time of day"
        hidden={kind !== 'daily'}
        value={at}
        onChange={(e) => editor.setField('at', e.target.value)}
      />
    </div>
  );
}

/** Scrolls the card into view and focuses the name each time the editor opens. */
function useFocusOnOpen(opened: number): void {
  useEffect(() => {
    if (!opened) return;
    document.getElementById('routine-form')?.scrollIntoView?.({ block: 'nearest' });
    document.getElementById('routine-name')?.focus();
  }, [opened]);
}

/** The name and the prompt. */
function TextFields({ editor }: RoutineEditorProps) {
  const { name, prompt } = useViewModel(editor);
  return (
    <>
      <label className="routine-field">
        Name
        <input
          id="routine-name"
          maxLength={EDITOR_LIMITS.nameChars}
          placeholder="Morning inbox"
          spellCheck={false}
          value={name}
          onChange={(e) => editor.setField('name', e.target.value)}
        />
      </label>
      <label className="routine-field">
        Prompt
        <textarea
          id="routine-prompt"
          rows={EDITOR_LIMITS.promptRows}
          maxLength={EDITOR_LIMITS.promptChars}
          placeholder="Open Gmail and summarize unread emails from today"
          value={prompt}
          onChange={(e) => editor.setField('prompt', e.target.value)}
        />
      </label>
    </>
  );
}

/** Run in the cloud: the server runs it on a cloud browser instead of a desktop. */
function CloudField({ editor }: RoutineEditorProps) {
  const { cloud } = useViewModel(editor);
  return (
    <label className="routine-cloud">
      <input id="routine-cloud" type="checkbox" checked={cloud} onChange={(e) => editor.setCloud(e.target.checked)} />
      <span>
        {EDITOR_TEXT.cloud}
        <small>{EDITOR_TEXT.cloudHint}</small>
      </span>
    </label>
  );
}

/** The editor card. */
export function RoutineEditor({ editor }: RoutineEditorProps) {
  const state = useViewModel(editor);
  useFocusOnOpen(state.opened);
  const editing = !!state.editing;
  return (
    <form
      className="routine-editor"
      id="routine-form"
      hidden={!state.open}
      onSubmit={(e) => (e.preventDefault(), void editor.submit())}
    >
      <div className="routine-editor-title" id="routine-form-title">
        {editing ? EDITOR_TEXT.editTitle : EDITOR_TEXT.newTitle}
      </div>
      <TextFields editor={editor} />
      <div className="routine-field">
        <span id="routine-repeat-label">Repeat</span>
        <ScheduleFields editor={editor} />
      </div>
      <CloudField editor={editor} />
      <p className="routine-error" id="routine-error" role="alert" hidden={!state.error}>
        {state.error}
      </p>
      <div className="routine-actions">
        <Button type="button" id="routine-cancel" onClick={() => editor.close()}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" id="routine-save">
          {editing ? EDITOR_TEXT.update : EDITOR_TEXT.create}
        </Button>
      </div>
    </form>
  );
}
