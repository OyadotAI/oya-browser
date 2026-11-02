/**
 * The routine editor's card (`#routine-form`): name, prompt or playbook and its
 * values, profile and schedule, with
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

/** What each run does: ask the agent the prompt below, or play one of the project's saved playbooks. */
function TaskField({ editor }: RoutineEditorProps) {
  const { playbook, playbooks } = useViewModel(editor);
  const listed = playbooks.map((p) => p.name);
  const names = playbook && !listed.includes(playbook) ? [playbook, ...listed] : listed;
  return (
    <label className="routine-field">
      {EDITOR_TEXT.task}
      <select id="routine-task" value={playbook} onChange={(e) => editor.setField('playbook', e.target.value)}>
        <option value="">{EDITOR_TEXT.askPrompt}</option>
        {names.length > 0 && (
          <optgroup label={EDITOR_TEXT.playGroup}>
            {names.map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </optgroup>
        )}
      </select>
    </label>
  );
}

/** The name and, unless it plays a playbook, the prompt. */
function TextFields({ editor }: RoutineEditorProps) {
  const { name, prompt, playbook } = useViewModel(editor);
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
      <TaskField editor={editor} />
      <label className="routine-field" hidden={!!playbook}>
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

/** One field per variable of the playbook it plays; a secret's says the saved one is used. */
function ValuesField({ editor }: RoutineEditorProps) {
  const { values } = useViewModel(editor);
  const chosen = editor.chosen();
  if (!chosen?.variables.length) return null;
  return (
    <fieldset className="routine-field routine-values" id="routine-values">
      <legend>{EDITOR_TEXT.values}</legend>
      {chosen.variables.map((name) => (
        <label key={name} className="routine-value">
          {name}
          <input
            data-variable={name}
            spellCheck={false}
            disabled={chosen.secrets.includes(name)}
            placeholder={chosen.secrets.includes(name) ? EDITOR_TEXT.secret : (chosen.defaults[name] ?? '')}
            value={chosen.secrets.includes(name) ? '' : (values[name] ?? '')}
            onChange={(e) => editor.setValue(name, e.target.value)}
          />
        </label>
      ))}
      <small>{EDITOR_TEXT.valuesHint}</small>
    </fieldset>
  );
}

/** The profile it runs as: the default, or one of the project's personas. */
function ProfileField({ editor }: RoutineEditorProps) {
  const { persona, personas, cloud } = useViewModel(editor);
  const listed = personas.some((p) => p.value === persona);
  const options = persona && !listed ? [{ value: persona, label: persona }, ...personas] : personas;
  return (
    <label className="routine-field">
      {EDITOR_TEXT.profile}
      <select id="routine-persona" value={persona} onChange={(e) => editor.setField('persona', e.target.value)}>
        <option value="">{EDITOR_TEXT.defaultProfile}</option>
        {options.map((p) => (
          <option key={p.value} value={p.value}>
            {p.label}
          </option>
        ))}
      </select>
      {persona && <small>{cloud ? EDITOR_TEXT.profileCloudHint : EDITOR_TEXT.profileHint}</small>}
    </label>
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
      <ValuesField editor={editor} />
      <ProfileField editor={editor} />
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
