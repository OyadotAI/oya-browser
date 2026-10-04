/**
 * The studio's small controls: a labelled field that reports its value when
 * it is left or Enter is pressed, and the message line under an action.
 */
import { StatusLine } from '../../../ui/index.ts';
import { useFieldDraft } from '../hooks/use-field-draft.ts';
import type { Slot } from '../model/constants.ts';
import type { Message } from '../model/studio-model.ts';

/** What a StudioField shows and reports. */
export interface StudioFieldProps {
  /** The words beside it. */
  label: string;
  /** Its value now. */
  value: string | number | undefined;
  /** Called with the new value: text, or for a number field a number or undefined when emptied. */
  onCommit: (value: string | number | undefined) => void;
  /** The input type (text by default). */
  type?: string;
  /** Its data-key (the label by default). */
  fieldKey?: string;
  /** Whether it is off. */
  disabled?: boolean;
}

/** A labelled input that reports a change. */
export function StudioField({ label, value, onCommit, type = 'text', fieldKey = label, disabled }: StudioFieldProps) {
  const parse = (text: string) => (type !== 'number' ? text : text === '' ? undefined : Number(text));
  const field = useFieldDraft(String(value ?? ''), (text) => onCommit(parse(text)));
  return (
    <label className="studio-field">
      <span>{label}</span>
      <input type={type} data-key={fieldKey} disabled={disabled} {...field} />
    </label>
  );
}

/** What a SlotStatus shows. */
export interface SlotStatusProps {
  /** The slot, which is its id. */
  slot: Slot;
  /** The message in it. */
  message: Message;
}

/** The message line under an action. */
export function SlotStatus({ slot, message }: SlotStatusProps) {
  return (
    <StatusLine className={'studio-status' + (message.error ? ' error' : '')} id={slot} live>
      {message.text}
    </StatusLine>
  );
}
