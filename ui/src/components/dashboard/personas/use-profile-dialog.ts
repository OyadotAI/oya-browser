/**
 * The account dialog's state: the display name being edited and how saving
 * it went. Email and role are shown but never edited.
 */
import { useCallback, useEffect, useState } from 'react';
import { useAuth } from '@/components/auth-provider';
import { updateProfile } from '@/lib/api';

/** The name being edited, and how the last save went. */
interface ProfileForm {
  /** The display name as typed. */
  name: string;
  /** A save is in flight. */
  saving: boolean;
  /** Why it failed; empty when it did not. */
  error: string;
  /** It succeeded and nothing was typed since. */
  saved: boolean;
}

/** Changes some fields of the form. */
type Patch = (p: Partial<ProfileForm>) => void;
/** Hands the saved profile to the auth provider. */
type Apply = ReturnType<typeof useAuth>['applyProfile'];

/** The form, reset to what is stored whenever the dialog opens. */
function useForm(open: boolean, stored: string | undefined) {
  const [form, setForm] = useState<ProfileForm>({ name: '', saving: false, error: '', saved: false });
  const patch: Patch = useCallback((p) => setForm((f) => ({ ...f, ...p })), []);
  // Reopening should show what is stored now, not what was typed and abandoned.
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- reopening shows what is stored now
    if (open) patch({ name: stored || '', error: '', saved: false });
  }, [open, stored, patch]);
  return { form, patch };
}

/** Saves the name, reporting progress and the outcome through `patch`. */
async function saveName(token: string | null, name: string, apply: Apply, patch: Patch) {
  if (!token || !name) return;
  patch({ saving: true, error: '', saved: false });
  try {
    apply(await updateProfile(token, name));
    patch({ saving: false, saved: true });
  } catch (e) {
    patch({ saving: false, error: e instanceof Error ? e.message : 'Could not save your profile' });
  }
}

/** Everything the account dialog shows and does. */
export function useProfileDialog(open: boolean) {
  const auth = useAuth();
  const { form, patch } = useForm(open, auth.user?.display_name);
  const name = form.name.trim();
  const dirty = name !== (auth.user?.display_name || '').trim();
  const edit = (value: string) => patch({ name: value, saved: false });
  const save = async () => (dirty ? saveName(auth.token, name, auth.applyProfile, patch) : undefined);
  return { ...auth, ...form, edit, dirty, save };
}

/** The dialog's state, as its parts receive it. */
export type ProfileState = ReturnType<typeof useProfileDialog>;
