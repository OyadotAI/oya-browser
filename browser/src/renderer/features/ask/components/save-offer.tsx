/**
 * "Save as playbook" under a reply (`.chat-save`): the button (disabled, with
 * why, for a run with nothing to replay), the name form (Enter saves, Escape
 * cancels), and the saved confirmation. It scrolls itself into view, since
 * the reply scrolled the list before it existed.
 */
import { useEffect, useRef, type KeyboardEvent } from 'react';
import { useViewModel } from '../../../hooks/index.ts';
import { Icon } from '../../../ui/index.ts';
import './save-offer.css';
import { ASK_TEXT, PLAYBOOK_NAME_MAX } from '../model/constants.ts';
import type { PlaybookOfferViewModel } from '../view-models/playbook-offer-view-model.ts';

/** What the offer's parts are given. */
interface OfferProps {
  /** Opens this saved playbook in the library. */
  onViewPlaybook?: (name: string) => void;
  /** The offer. */
  offer: PlaybookOfferViewModel;
}

/** The id of the note saying why the offer is disabled. */
const NOTE_ID = 'chat-save-note';

/** The "Save as playbook" button, disabled with its reason when the run cannot be replayed. */
function OfferButton({ offer }: OfferProps) {
  const { canSave } = useViewModel(offer);
  return (
    <>
      <button
        className="chat-save-button"
        disabled={!canSave}
        aria-describedby={canSave ? undefined : NOTE_ID}
        onClick={() => offer.openForm()}
      >
        <Icon name="playbook" />
        <span>{ASK_TEXT.saveAsPlaybook}</span>
      </button>
      {!canSave && (
        <span className="chat-save-note" id={NOTE_ID}>
          {ASK_TEXT.nothingToReplay}
        </span>
      )}
    </>
  );
}

/** The name form: selected on open, focused again after a failure. */
function OfferForm({ offer }: OfferProps) {
  const { name, error, stage } = useViewModel(offer);
  const field = useRef<HTMLInputElement>(null);
  const busy = stage === 'saving';
  useEffect(() => void field.current?.select(), []);
  useEffect(() => void (error && field.current?.focus()), [error]);
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Enter' && !e.nativeEvent.isComposing) void offer.save();
    else if (e.key === 'Escape') offer.cancel();
    else return;
    e.preventDefault();
  };
  return (
    <>
      <p className="chat-save-copy">{ASK_TEXT.saveExplainer}</p>
      <div className="chat-save-row">
        <input
          className="chat-save-name"
          ref={field}
          value={name}
          maxLength={PLAYBOOK_NAME_MAX}
          spellCheck={false}
          aria-label={ASK_TEXT.playbookName}
          disabled={busy}
          onChange={(e) => offer.setName(e.target.value)}
          onKeyDown={onKey}
        />
        <button className="chat-save-confirm" disabled={busy} onClick={() => void offer.save()}>
          Save
        </button>
        <button className="chat-save-cancel" disabled={busy} onClick={() => offer.cancel()}>
          Cancel
        </button>
      </div>
      <p className="chat-save-error">{error}</p>
    </>
  );
}

/** The offer under a reply. */
export function SaveOffer({ offer, onViewPlaybook }: OfferProps) {
  const { stage, savedAs } = useViewModel(offer);
  const box = useRef<HTMLDivElement>(null);
  const formOpen = stage !== 'offer';
  useEffect(() => void box.current?.scrollIntoView?.({ block: 'end' }), [formOpen]);
  return (
    <div className={stage === 'saved' ? 'chat-save saved' : 'chat-save'} ref={box}>
      {stage === 'offer' && <OfferButton offer={offer} />}
      {(stage === 'form' || stage === 'saving') && <OfferForm offer={offer} />}
      {stage === 'saved' && (
        <p className="chat-save-done">
          <Icon name="check" />
          <span>{`Saved as playbook “${savedAs}”`}</span>
          {onViewPlaybook && <button onClick={() => onViewPlaybook(savedAs)}>View playbook</button>}
        </p>
      )}
    </div>
  );
}
