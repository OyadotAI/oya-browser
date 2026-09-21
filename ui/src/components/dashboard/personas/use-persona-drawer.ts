/**
 * The profile drawer's state: its editable fields, the proxies it may pin to,
 * the sign-in and second-factor drafts, and which action is running.
 */
import { useCallback, useEffect, useState } from 'react';
import type { BrowserRow, Persona } from '../types';
import { useToast } from '../toast';
import { newMfa, type MfaDraft } from './mfa';
import { fieldsOf, type CredentialDraft, type DrawerFields, type ProxyChoice } from './model';
import { fetchProxyChoices } from './persona-api';
import { useBusy } from './use-busy';

/** What the drawer needs from the tab that opens it. */
export interface DrawerProps {
  /** The persona shown; null keeps the drawer closed. */
  persona: Persona | null;
  /** Closes the drawer. */
  onClose: () => void;
  /** Key the persona belongs to. */
  apiKey: string;
  /** Every connected browser, to show the ones running as this persona. */
  browsers: BrowserRow[];
  /** Asks the tab to reload after a change. */
  onChanged: () => void;
  /** Shows the fleet filtered to this persona. */
  onShowBrowsers: (personaId: string) => void;
  /** The clock the relative times are measured against. */
  now: number;
}

/** An empty sign-in form. */
const BLANK_CREDENTIAL: CredentialDraft = { domain: '', username: '', password: '' };
/** The drawer's fields before any persona has been shown. */
const BLANK_FIELDS: DrawerFields = { name: '', cap: '', geo: '', pin: '' };

/** A stable setter that changes some fields of an object state. */
function usePatch<T>(setState: (update: (prev: T) => T) => void) {
  return useCallback((patch: Partial<T>) => setState((prev) => ({ ...prev, ...patch })), [setState]);
}

/** The editable fields and second-factor draft, refilled whenever another persona opens. */
function useDrawerForm(persona: Persona | null, apiKey: string) {
  const [fields, setFields] = useState<DrawerFields>(BLANK_FIELDS);
  const [mfa, setMfa] = useState<MfaDraft>(newMfa());
  useEffect(() => {
    if (!persona) return;
    setFields(fieldsOf(persona));
    setMfa(newMfa());
    // Refreshing profile counts must not overwrite a form being edited.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [persona?.id, apiKey]);
  return { fields, set: usePatch(setFields), mfa, setMfa };
}

/** The proxies the exit picker offers, fetched whenever another persona opens; none on failure. */
function useProxyChoices(persona: Persona | null, apiKey: string) {
  const [proxies, setProxies] = useState<ProxyChoice[]>([]);
  useEffect(() => {
    if (persona) fetchProxyChoices(apiKey).then(setProxies, () => setProxies([]));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [persona?.id, apiKey]);
  return proxies;
}

/** Runs a drawer action: busy while it runs, an optional success toast, then a reload. */
function useAct(onChanged: () => void) {
  const toast = useToast();
  const { busy, run } = useBusy();
  const succeeded = (done?: string) => (done && toast(done, 'success'), onChanged());
  const act = (what: string, fn: () => Promise<unknown>, done?: string) =>
    run(what, () => fn().then(() => succeeded(done)));
  return { busy, act, toast };
}

/** Everything the drawer shows and changes. The sign-in draft outlives the drawer closing, as it always has. */
export function usePersonaDrawer({ persona, apiKey, onChanged }: DrawerProps) {
  const form = useDrawerForm(persona, apiKey);
  const proxies = useProxyChoices(persona, apiKey);
  const [cred, setCred] = useState<CredentialDraft>(BLANK_CREDENTIAL);
  const [confirmDelete, setConfirmDelete] = useState(false);
  return { ...form, proxies, cred, setCred, confirmDelete, setConfirmDelete, ...useAct(onChanged) };
}

/** The drawer's state, as its sections receive it. */
export type DrawerState = ReturnType<typeof usePersonaDrawer>;

/** What each drawer section works with: the state, the persona shown, and the drawer's props. */
export interface DrawerCtx {
  /** The drawer's state. */
  d: DrawerState;
  /** The persona shown. */
  p: Persona;
  /** The drawer's props. */
  props: DrawerProps;
}
