/**
 * The start-browser dialog's state: the form, whether the chosen provider is
 * ready, and `start`, which starts the browsers and reports how it went.
 */
import { useState } from 'react';
import { useToast } from '../toast';
import type { ShowToast } from '../toast/types';
import { startMany, type StartForm } from './start-run';

/** A provider as the dialog lists it. */
export interface ProviderOption {
  /** Provider id. */
  id: string;
  /** Display name. */
  label: string;
  /** Whether this key can start browsers on it. */
  configured: boolean;
}

/** What the dialog is given. */
export interface StartOptions {
  /** The key browsers start under. */
  apiKey: string;
  /** The key's default provider. */
  defaultProvider: string;
  /** Every provider and whether it is ready. */
  providers: ProviderOption[];
  /** At least one browser started. */
  onStarted: () => void;
  /** Closes the dialog. */
  onClose: () => void;
}

/** The form fields and their setters. */
export function useStartForm() {
  const [persona, setPersona] = useState('default');
  const [provider, setProvider] = useState('');
  const [name, setName] = useState('');
  const [count, setCount] = useState(1);
  return { persona, setPersona, provider, setProvider, name, setName, count, setCount };
}

/** The whole dialog: form, readiness, busy and error state, and `start`. */
export function useStartBrowser(opts: StartOptions) {
  const toast = useToast();
  const form = useStartForm();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const selectedProvider = form.provider || opts.defaultProvider;
  const ready = opts.providers.some((p) => p.id === selectedProvider && p.configured);
  const start = () => runStart({ ...opts, form, busy, ready, setBusy, setError, toast });
  return { form, busy, error, setError, ready, selectedProvider, start };
}

/** Everything one start needs. */
interface StartRun extends StartOptions {
  /** The form as submitted. */
  form: StartForm;
  /** A start already running: ignore the click. */
  busy: boolean;
  /** The provider can start browsers. */
  ready: boolean;
  /** Marks the dialog busy. */
  setBusy: (busy: boolean) => void;
  /** Shows an error in the dialog. */
  setError: (error: string) => void;
  /** Raises a toast. */
  toast: ShowToast;
}

/** Starts the browsers, then reports. Ignored while busy or when the provider is not ready. */
async function runStart(run: StartRun) {
  if (run.busy || !run.ready) return;
  run.setError('');
  run.setBusy(true);
  const { ok, lastErr } = await startMany(run.apiKey, run.form);
  run.setBusy(false);
  settle(run, ok, lastErr);
}

/** Toasts what started; keeps the dialog open with the error if something failed, closes it otherwise. */
function settle(run: StartRun, ok: number, lastErr: string) {
  if (ok) {
    run.toast(ok === 1 ? 'Browser starting' : `${ok} browsers starting`, 'success');
    run.onStarted();
  }
  if (lastErr) run.setError(ok ? `${ok} started. ${lastErr}` : lastErr);
  else if (ok) run.onClose();
}
