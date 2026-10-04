/**
 * A studio field's typed text: shown while typing, and reported once, when it
 * changed and the field is left or Enter is pressed (as a native change does).
 */
import { useState, type ChangeEvent, type Dispatch, type KeyboardEvent, type SetStateAction } from 'react';

/** The text being typed into a field, and the value it started from. */
interface Typing {
  /** The value when typing started. */
  from: string;
  /** What is typed now. */
  text: string;
}

/** Reports a changed draft; `keep` goes on typing from it (Enter), otherwise typing ends (leaving). */
const reporter =
  (typing: Typing | null, setTyping: SetTyping, commit: (text: string) => void) =>
  (keep: boolean): void => {
    if (typing && typing.text !== typing.from) commit(typing.text);
    setTyping(keep && typing ? { from: typing.text, text: typing.text } : null);
  };

/** What sets the draft. */
type SetTyping = Dispatch<SetStateAction<Typing | null>>;

/** The input's handlers: focus starts a draft, typing changes it, leaving or Enter reports it. */
function draftHandlers(value: string, typing: Typing | null, setTyping: SetTyping, commit: (text: string) => void) {
  const report = reporter(typing, setTyping, commit);
  return {
    onFocus: () => setTyping({ from: value, text: value }),
    onChange: (event: ChangeEvent<HTMLInputElement>) =>
      setTyping({ from: typing?.from ?? value, text: event.target.value }),
    onBlur: () => report(false),
    onKeyDown: (event: KeyboardEvent) => event.key === 'Enter' && report(true),
  };
}

/** The input props that show `value`, or the draft while typing, and call `commit` with a changed draft. */
export function useFieldDraft(value: string, commit: (text: string) => void) {
  const [typing, setTyping] = useState<Typing | null>(null);
  return { value: typing ? typing.text : value, ...draftHandlers(value, typing, setTyping, commit) };
}
