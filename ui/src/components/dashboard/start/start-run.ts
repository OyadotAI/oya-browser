/**
 * Starting browsers: one request per browser, in order, stopping at the first
 * failure so a quota or provider error is not repeated `count` times.
 */
import { api, errorMessage } from '@/lib/api-client';
import { providerLabel } from '../types';
import { MAX_BROWSERS_PER_START } from './constants';

/** What the dialog asks for. */
export interface StartForm {
  /** Persona id, or "default" / "auto". */
  persona: string;
  /** Provider for these browsers only; empty uses the key default. */
  provider: string;
  /** Optional name; numbered when starting several. */
  name: string;
  /** How many to start. */
  count: number;
}

/** How a run went: how many started, and the error that stopped it, if any. */
export interface StartOutcome {
  /** Browsers started. */
  ok: number;
  /** Why it stopped early; empty when it did not. */
  lastErr: string;
}

/** Request body for browser `i` of a run. Browsers are numbered from 1 when there are several. */
export function startBody({ persona, provider, name, count }: StartForm, i: number) {
  return {
    persona,
    ...(provider ? { provider } : {}),
    ...(name ? { name: count > 1 ? `${name} ${i + 1}` : name } : {}),
  };
}

/** Starts one browser; answers the error message, or empty on success. */
async function startOne(apiKey: string, body: object): Promise<string> {
  try {
    await api('/browsers/start', { key: apiKey, method: 'POST', body });
    return '';
  } catch (err) {
    return errorMessage(err);
  }
}

/** Starts the form's browsers one after another, stopping at the first failure. */
export async function startMany(apiKey: string, form: StartForm): Promise<StartOutcome> {
  let ok = 0;
  let lastErr = '';
  while (ok < form.count && !lastErr) {
    lastErr = await startOne(apiKey, startBody(form, ok));
    if (!lastErr) ok++;
  }
  return { ok, lastErr };
}

/** Keeps a typed count between 1 and the per-click maximum; anything unreadable is 1. */
export const clampCount = (value: string) => Math.min(MAX_BROWSERS_PER_START, Math.max(1, Number(value) || 1));

/** Why the chosen provider cannot start a browser, in words the user can act on. */
export function unavailableReason(provider: string) {
  return provider === 'oya-cloud' || provider === 'oya-selfhosted'
    ? 'Cloud browsers are unavailable on this server. The server operator needs to finish cloud setup and provide a public connection address. You can keep using your connected desktop or choose a ready provider below.'
    : `${providerLabel(provider)} is not configured. Add its connection details in Settings, or choose a ready provider below.`;
}
