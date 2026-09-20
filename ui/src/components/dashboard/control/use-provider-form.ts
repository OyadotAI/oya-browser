/**
 * The Add provider form: whether it is open, what has been typed, and what
 * opening, cancelling and submitting it do.
 */
import { useCallback, useState, type Dispatch, type FormEvent, type SetStateAction } from 'react';
import { EMPTY_DRAFT } from './constants';
import { providerBody } from './format';
import { post } from './requests';
import type { ProviderDraft } from './types';

/** What the form needs from the action runner. */
interface FormActions {
  /** Runs an action with a busy label, then reloads. */
  act: (label: string, fn: () => Promise<unknown>) => Promise<void>;
  /** Shows a success message. */
  setNotice: (notice: string) => void;
  /** Clears the last action error. */
  clearError: () => void;
}

/** Patches the draft. */
type Edit = (patch: Partial<ProviderDraft>) => void;

/** Said once a provider is saved: nothing verifies it until a client connects. */
const SAVED_NOTICE = 'Provider saved. Its first CDP connection will verify that the endpoint and credentials work.';

/** Open state and draft of the Add provider form, with its handlers. */
export function useProviderForm(apiKey: string, actions: FormActions) {
  const [showAdd, setShowAdd] = useState(false);
  const [draft, setDraft] = useState<ProviderDraft>(EMPTY_DRAFT);
  const edit = useCallback((patch: Partial<ProviderDraft>) => setDraft((d) => ({ ...d, ...patch })), []);
  const reset = resetter(setShowAdd, edit, actions.clearError);
  const submitAdd = submitter(apiKey, draft, actions, () => (setShowAdd(false), setDraft(EMPTY_DRAFT)));
  return { showAdd, draft, edit, toggleAdd: () => reset((v) => !v), cancelAdd: () => reset(false), submitAdd };
}

/** Opens or closes the form, dropping any typed vendor key and the last error. */
function resetter(setShowAdd: Dispatch<SetStateAction<boolean>>, edit: Edit, clearError: () => void) {
  return (open: SetStateAction<boolean>) => {
    setShowAdd(open);
    edit({ apiKey: '' });
    clearError();
  };
}

/** Saves the draft as a provider, then closes and clears the form. */
function submitter(apiKey: string, draft: ProviderDraft, actions: FormActions, done: () => void) {
  return (e: FormEvent) => {
    e.preventDefault();
    void actions.act('add-provider', async () => {
      await post(apiKey, '/gateway/providers', providerBody(draft));
      done();
      actions.setNotice(SAVED_NOTICE);
    });
  };
}
