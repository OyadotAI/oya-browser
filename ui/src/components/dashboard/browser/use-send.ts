/**
 * Console input to the browser. It is human input: the server refuses it unless
 * a person holds control, and a refusal or failure is toasted.
 */
import { useCallback } from 'react';
import { api, errorMessage } from '@/lib/api-client';
import type { useToast } from '../toast';
import type { InputResult, Send } from './types';

/** Which browser input goes to, and where failures are reported. */
export interface SendTarget {
  /** The browser. */
  browserId: string;
  /** The project key. */
  apiKey: string;
  /** Shows a toast. */
  toast: ReturnType<typeof useToast>;
}

/** POSTs one input command to the browser's control session. */
const postInput = ({ browserId, apiKey }: SendTarget, action: string, params: Record<string, unknown>) =>
  api<InputResult>(`/control/sessions/${browserId}/input`, { key: apiKey, method: 'POST', body: { action, params } });

/** Sends a command; toasts the server's refusal or the network error, and never throws. */
export async function sendInput(t: SendTarget, action: string, params: Record<string, unknown> = {}) {
  try {
    const r = await postInput(t, action, params);
    if (r.ok === false) t.toast(r.error || `${action} failed`, 'error');
    return r;
  } catch (err) {
    t.toast(errorMessage(err), 'error');
    return { ok: false } as InputResult;
  }
}

/** A stable `send` for the browser. */
export function useSend(browserId: string, apiKey: string, toast: ReturnType<typeof useToast>): Send {
  return useCallback(
    (action, params = {}) => sendInput({ browserId, apiKey, toast }, action, params),
    [browserId, apiKey, toast],
  );
}
