/**
 * The new-profile form's state: the draft, the device choices on offer, a
 * debounced preview of the device those choices give, and creation.
 */
import { useCallback, useEffect, useMemo, useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import type { Persona } from '../types';
import { useToast } from '../toast';
import { DEFAULT_CONCURRENT, PREVIEW_DEBOUNCE_MS } from './constants';
import { newMfa } from './mfa';
import type { Fingerprint, Options, PersonaDraft } from './model';
import { createPersona, fetchOptions, fetchPreview } from './persona-api';
import { useBusy } from './use-busy';

/** A blank form: every device choice left to the server, no second factor. */
const blankDraft = (): PersonaDraft => ({
  name: '',
  platform: 'auto',
  timezone: 'auto',
  locale: 'auto',
  geo: '',
  cap: DEFAULT_CONCURRENT,
  mfa: newMfa(''),
});

/** Changes some fields of a draft. */
export type SetDraft = (patch: Partial<PersonaDraft>) => void;

/** A timezone only makes sense for the platform that carries it: picking a platform puts both back on auto. */
const withPlatformReset = (patch: Partial<PersonaDraft>) =>
  'platform' in patch ? { ...patch, timezone: 'auto', locale: 'auto' } : patch;

/** The draft, blanked each time the form opens. */
function useDraft(open: boolean, apiKey: string) {
  const [draft, setDraft] = useState<PersonaDraft>(blankDraft);
  const set: SetDraft = useCallback((patch) => setDraft((d) => ({ ...d, ...withPlatformReset(patch) })), []);
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- reopening starts a fresh form
    if (open) setDraft(blankDraft());
  }, [open, apiKey]);
  return { draft, set };
}

/** The device preferences to send: only the choices not left on auto. */
function usePrefs({ platform, timezone, locale }: PersonaDraft) {
  return useMemo(
    () => ({
      ...(platform !== 'auto' ? { platform } : {}),
      ...(timezone !== 'auto' ? { timezone } : {}),
      ...(locale !== 'auto' ? { locale } : {}),
    }),
    [platform, timezone, locale],
  );
}

/** The platforms, timezones and locales on offer, fetched each time the form opens. */
function useOptions(open: boolean, apiKey: string) {
  const toast = useToast();
  const [opts, setOpts] = useState<Options | null>(null);
  useEffect(() => {
    if (open) fetchOptions(apiKey).then(setOpts, (e) => toast(errorMessage(e), 'error'));
  }, [open, apiKey, toast]);
  return opts;
}

/** Asks for the preview once the choices have settled; the returned cleanup cancels the wait or the request. */
function debouncedPreview(apiKey: string, prefs: Record<string, string>, onPreview: (fp: Fingerprint) => void) {
  const ctl = new AbortController();
  const fetchLater = () => fetchPreview(apiKey, prefs, ctl.signal).then(onPreview, () => {});
  const t = setTimeout(fetchLater, PREVIEW_DEBOUNCE_MS);
  return () => {
    clearTimeout(t);
    ctl.abort();
  };
}

/** The device these preferences give, asked for once the choices settle; a stale answer is dropped. */
function useFingerprintPreview(open: boolean, apiKey: string, prefs: Record<string, string>) {
  const [preview, setPreview] = useState<Fingerprint | null>(null);
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- reopening starts from an empty preview
    if (open) setPreview(null);
  }, [open, apiKey]);
  useEffect(() => (open ? debouncedPreview(apiKey, prefs, setPreview) : undefined), [open, apiKey, prefs]);
  return preview;
}

/** What opening, filling in and submitting the form needs. */
interface FormArgs {
  /** Whether the dialog is showing. */
  open: boolean;
  /** Key the profile is created under. */
  apiKey: string;
  /** Hears about the new profile. */
  onCreated: (p: Persona) => void;
  /** Closes the dialog. */
  onClose: () => void;
}

/** Creates the profile from the draft, then announces it and closes the form. */
function useCreate({ apiKey, onCreated, onClose }: FormArgs, draft: PersonaDraft, prefs: Record<string, string>) {
  const toast = useToast();
  const { busy, run } = useBusy();
  const created = (p: Persona) => (toast(`Created ${p.name}`, 'success'), onCreated(p), onClose());
  const create = () => run('create', async () => created(await createPersona(apiKey, draft, prefs)));
  return { busy: busy !== null, create };
}

/** Everything the new-profile form shows and does. */
export function usePersonaForm(args: FormArgs) {
  const { draft, set } = useDraft(args.open, args.apiKey);
  const opts = useOptions(args.open, args.apiKey);
  const prefs = usePrefs(draft);
  const preview = useFingerprintPreview(args.open, args.apiKey, prefs);
  return { draft, set, opts, preview, ...useCreate(args, draft, prefs) };
}
