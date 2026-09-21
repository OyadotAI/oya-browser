/**
 * The proxies dialog's state: the proxy list, the add form, which row is
 * asking to confirm its removal, and which action is running.
 */
import { useCallback, useEffect, useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import { useToast } from '../toast';
import { DEFAULT_MAX_PROFILES } from './constants';
import type { ProxyDraft, ProxyRow } from './model';
import { listProxies } from './persona-api';
import { useBusy } from './use-busy';

/** What the dialog needs from the tab that opens it. */
export interface ProxiesProps {
  /** Whether it is showing. */
  open: boolean;
  /** Closes it. */
  onClose: () => void;
  /** Key the proxies belong to. */
  apiKey: string;
  /** Asks the tab to reload after a change. */
  onChanged: () => void;
}

/** An empty add form: residential, one profile per proxy. */
const blankDraft = (): ProxyDraft => ({ label: '', url: '', geo: '', kind: 'residential', max: DEFAULT_MAX_PROFILES });

/** The proxy list, loaded each time the dialog opens; a failed load is toasted. */
function useProxyRows(open: boolean, apiKey: string) {
  const toast = useToast();
  const [rows, setRows] = useState<ProxyRow[]>([]);
  const failed = useCallback((e: unknown) => toast(errorMessage(e), 'error'), [toast]);
  const load = useCallback(() => listProxies(apiKey).then(setRows, failed), [apiKey, failed]);
  useEffect(() => {
    if (open) load();
  }, [open, load]);
  return { rows, load };
}

/** The add form, cleared each time the dialog opens. */
function useProxyForm(open: boolean, load: () => Promise<unknown>) {
  const [draft, setDraft] = useState<ProxyDraft>(blankDraft);
  const set = useCallback((patch: Partial<ProxyDraft>) => setDraft((d) => ({ ...d, ...patch })), []);
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- reopening starts a fresh form
    if (open) setDraft(blankDraft());
  }, [open, load]);
  return { draft, set };
}

/** The proxy asking "Remove?", forgotten each time the dialog opens. */
function useConfirming(open: boolean, load: () => Promise<unknown>) {
  const [confirming, setConfirming] = useState<string | null>(null);
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- reopening forgets a half-made removal
    if (open) setConfirming(null);
  }, [open, load]);
  return { confirming, setConfirming };
}

/** Everything the proxies dialog shows and changes. */
export function useProxies(props: ProxiesProps) {
  const { rows, load } = useProxyRows(props.open, props.apiKey);
  const form = { ...useProxyForm(props.open, load), ...useConfirming(props.open, load) };
  const toast = useToast();
  return { rows, load, ...form, ...useBusy(), toast, props };
}

/** The dialog's state, as its actions and parts receive it. */
export type ProxiesState = ReturnType<typeof useProxies>;

/** Props for a part of the dialog. */
export interface PartProps {
  /** The dialog's state. */
  s: ProxiesState;
}

/** Props for a part about one proxy. */
export interface ProxyPartProps extends PartProps {
  /** The proxy. */
  p: ProxyRow;
}
