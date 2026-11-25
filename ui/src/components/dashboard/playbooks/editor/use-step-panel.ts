/**
 * The visual view's state: which step's panel is open and the edit not yet
 * confirmed. Confirm and Delete rewrite the definition text, the same way the
 * Variables pane does, so the dirty dot, Save and the tester see one playbook.
 * Opening a card or switching to JSON never writes, so the text stays as it was.
 */
import { useEffect, useRef, useState } from 'react';
import { stepsOf, toText, withStep, withoutStep, type Step } from './definition';
import { STEP_FIELDS, type StepField } from './editor-text';
import type { Definition } from './editor-api';

/** The panel's unconfirmed values, as typed. */
export type Draft = Partial<Record<StepField, string>>;

/** Fields replay reads as numbers, so a typed value is written back as one. */
const NUMERIC: ReadonlySet<StepField> = new Set(['amount']);

/** The id of step `i`'s card, so focus can go back to it. */
export const cardId = (i: number) => `step-card-${i}`;

/** The id of the End pill, where focus goes when the last step is deleted. */
export const END_ID = 'step-end';

/** The editable fields a step has, as text for the inputs. */
export const draftOf = (step: Step): Draft =>
  Object.fromEntries(
    (Object.keys(STEP_FIELDS) as StepField[]).filter((k) => step[k] !== undefined).map((k) => [k, String(step[k])]),
  );

/** The draft as step fields, numbers as numbers. */
const editsOf = (draft: Draft) =>
  Object.fromEntries(Object.entries(draft).map(([k, v]) => [k, NUMERIC.has(k as StepField) ? Number(v) : v]));

/**
 * Whether the draft can be confirmed. An empty amount would be written as 0, and
 * the server drops a navigate step with no url on save, so either would silently
 * change the playbook.
 */
export const canConfirm = (draft: Draft) =>
  (draft.url === undefined || draft.url.trim() !== '') &&
  (draft.amount === undefined || (draft.amount.trim() !== '' && Number.isFinite(Number(draft.amount))));

/** Moves focus to an element once React has drawn it. */
function useFocusAfterRender() {
  const next = useRef<string | null>(null);
  useEffect(() => {
    const id = next.current;
    next.current = null;
    if (id) document.getElementById(id)?.focus();
  });
  return (id: string) => void (next.current = id);
}

/** What the panel's actions work on. */
interface Ctx {
  /** The parsed definition. */
  definition: Definition;
  /** The open step's index. */
  open: number;
  /** The unconfirmed values. */
  draft: Draft;
  /** Replaces the editor text. */
  write: (d: Definition) => void;
  /** Closes the panel, focusing the element with this id. */
  close: (focusId: string) => void;
}

/** Where focus goes once step `i` is deleted: the step that took its place, or End. */
const afterDelete = (c: Ctx) => (c.open < stepsOf(c.definition).length - 1 ? cardId(c.open) : END_ID);

/** Cancel, Confirm and Delete for the open step. */
const actionsOf = (c: Ctx) => ({
  cancel: () => c.close(cardId(c.open)),
  confirm: () => (c.write(withStep(c.definition, c.open, editsOf(c.draft))), c.close(cardId(c.open))),
  remove: () => (c.write(withoutStep(c.definition, c.open)), c.close(afterDelete(c))),
});

/** Which step is open, and the step as it was when opened. */
interface Opened {
  /** Its index. */
  i: number;
  /** Its JSON when the panel opened. */
  as: string;
}

/**
 * The open step's index, or null once that step is not the one the panel opened
 * (Discard, a JSON edit or a delete changed it), so a draft never lands on another step.
 */
const liveIndex = (definition: Definition, opened: Opened | null) =>
  opened && JSON.stringify(stepsOf(definition)[opened.i]) === opened.as ? opened.i : null;

/** Opens step `i`: remembers it as it is now, and fills the draft from it. */
function opening(step: Step, i: number, setOpened: (o: Opened) => void, setDraft: (d: Draft) => void) {
  setOpened({ i, as: JSON.stringify(step) });
  setDraft(draftOf(step));
}

/** The panel's unconfirmed values, and a setter for one field. */
function useDraft() {
  const [draft, setDraft] = useState<Draft>({});
  const setField = (k: StepField, v: string) => setDraft((d) => ({ ...d, [k]: v }));
  return { draft, setDraft, setField };
}

/** The open step, its draft, and Open, Cancel, Confirm and Delete. */
export function useStepPanel(definition: Definition, onChange: (text: string) => void) {
  const [opened, setOpened] = useState<Opened | null>(null);
  const { draft, setDraft, setField } = useDraft();
  const focus = useFocusAfterRender();
  const open = liveIndex(definition, opened);
  const close = (focusId: string) => (setOpened(null), focus(focusId));
  const openStep = (i: number) => opening(stepsOf(definition)[i], i, setOpened, setDraft);
  const acts = actionsOf({ definition, open: open ?? -1, draft, write: (d) => onChange(toText(d)), close });
  return { open, draft, setField, openStep, ...acts };
}

/** What the hook gives the views. */
export type StepPanel = ReturnType<typeof useStepPanel>;
