/**
 * The save card (#record-finish): shown once there are steps and no
 * recording, with where the workflow is, the name and Save, the hint, and the
 * description. Right after a recording it comes into view with the name
 * focused when empty.
 */
import { useEffect, useRef, type KeyboardEvent, type RefObject } from 'react';
import { SlotStatus } from './fields.tsx';
import { NAME_MAX_LENGTH } from '../model/constants.ts';
import {
  finishCopy,
  finishTitle,
  isDisabled,
  saveHint,
  saveLabel,
  saveTitle,
  studioMode,
} from '../model/studio-model.ts';
import type { StudioPartProps } from './record-pane.tsx';
import { Button } from '../../../ui/index.ts';

/** Scrolls the card into view and focuses an empty name, once, when the studio asks. */
function useReveal({ vm, state }: StudioPartProps, shown: boolean) {
  const card = useRef<HTMLElement>(null);
  const name = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (!shown || !state.revealFinish) return;
    card.current?.scrollIntoView?.({ block: 'nearest' });
    if (!name.current?.value) name.current?.focus({ preventScroll: true });
    vm.finishRevealed();
  }, [vm, shown, state.revealFinish]);
  return { card, name };
}

/** The save card. */
export function FinishCard({ vm, state }: StudioPartProps) {
  const draft = state.snapshot?.draft;
  const shown = !!draft?.steps.length && !studioMode(state).recording;
  const { card, name } = useReveal({ vm, state }, shown);
  return (
    <section
      className="record-finish"
      id="record-finish"
      hidden={!shown}
      aria-labelledby="record-finish-title"
      ref={card}
    >
      <h3 id="record-finish-title">{finishTitle(state)}</h3>
      <p id="record-finish-copy">{draft ? finishCopy(state, draft) : ''}</p>
      <SaveRow vm={vm} state={state} nameRef={name} />
      <SlotStatus slot="save-result" message={state.messages['save-result']} />
      <p className="finish-connection" id="record-save-hint">
        {saveHint(state)}
      </p>
      <DescriptionField vm={vm} state={state} />
    </section>
  );
}

/** The description, folded away. */
export function DescriptionField({ vm, state }: StudioPartProps) {
  return (
    <details className="studio-details">
      <summary>Description</summary>
      <label className="sr-only" htmlFor="record-desc">
        Description
      </label>
      <input
        id="record-desc"
        placeholder="What does this workflow do?"
        value={state.description}
        disabled={isDisabled(state, 'record-desc')}
        onFocus={() => vm.focusField('description')}
        onChange={(event) => vm.typeField('description', event.target.value)}
        onBlur={() => vm.blurField()}
      />
    </details>
  );
}

/** What the name-and-save row is drawn from. */
export interface SaveRowProps extends StudioPartProps {
  /** The name field, which the card focuses when it comes into view. */
  nameRef: RefObject<HTMLInputElement | null>;
}

/** The playbook name and Save; Enter in the name saves when Save is on. */
export function SaveRow({ vm, state, nameRef }: SaveRowProps) {
  const draft = state.snapshot?.draft;
  const nameKey = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== 'Enter' || isDisabled(state, 'record-save')) return;
    event.preventDefault();
    event.currentTarget.blur();
    void vm.actions.save();
  };
  return (
    <div className="finish-save">
      <label className="sr-only" htmlFor="record-name">
        Playbook name
      </label>
      <input
        id="record-name"
        ref={nameRef}
        placeholder="playbook-name"
        maxLength={NAME_MAX_LENGTH}
        spellCheck={false}
        value={state.name}
        disabled={isDisabled(state, 'record-name')}
        onFocus={() => vm.focusField('name')}
        onChange={(event) => vm.typeField('name', event.target.value)}
        onBlur={() => vm.blurField()}
        onKeyDown={nameKey}
      />
      <Button
        variant="primary"
        id="record-save"
        title={saveTitle(state.connected)}
        disabled={isDisabled(state, 'record-save')}
        onClick={() => void vm.actions.save()}
      >
        {draft ? saveLabel(state, draft) : 'Save playbook'}
      </Button>
    </div>
  );
}
