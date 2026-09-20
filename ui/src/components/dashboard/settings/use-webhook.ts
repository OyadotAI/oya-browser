/**
 * The webhook panel's state and actions. It saves itself, like the Slack
 * panel — the endpoint is live the moment it is saved.
 */
import { useCallback, useEffect, useState } from 'react';
import { api, errorMessage } from '@/lib/api-client';
import { useToast } from '../toast';
import { withBusy } from './busy';
import type { SavedHook, WebhookConfig } from './webhook-types';

/** Where a load puts its results. */
interface LoadTarget {
  /** The loaded config. */
  setConfig: (config: WebhookConfig) => void;
  /** The URL field. */
  setUrl: (url: string) => void;
  /** The chosen event types. */
  setTypes: (types: string[]) => void;
  /** A failure. */
  setError: (error: string) => void;
}

/** Shows the config, and fills the form from the saved endpoint when there is one. */
function applyLoaded(next: WebhookConfig, target: LoadTarget) {
  target.setConfig(next);
  if (!next.hook) return;
  target.setUrl(next.hook.url);
  target.setTypes(next.hook.types);
}

/** Loads the webhook; a failure shows as the panel's error. */
async function fetchWebhook(apiKey: string, target: LoadTarget) {
  try {
    applyLoaded(await api<WebhookConfig>('/control/webhook', { key: apiKey }), target);
  } catch (err) {
    target.setError(errorMessage(err));
  }
}

/** The loaded config and the form over it; loads on mount and whenever `load` is called. */
function useWebhookData(apiKey: string) {
  const [config, setConfig] = useState<WebhookConfig | null>(null);
  const [url, setUrl] = useState('');
  const [types, setTypes] = useState<string[]>([]);
  const [error, setError] = useState('');
  const load = useCallback(() => fetchWebhook(apiKey, { setConfig, setUrl, setTypes, setError }), [apiKey]);
  useEffect(() => void load(), [load]);
  return { config, url, setUrl, types, setTypes, error, setError, load };
}

/** What the actions work with. */
type ActionContext = ReturnType<typeof useWebhookData> & {
  /** The key. */
  apiKey: string;
  /** Confirmation toasts. */
  toast: ReturnType<typeof useToast>;
  /** Shows a freshly minted signing secret. */
  setSecret: (secret: string) => void;
};

/** Saves the endpoint (rolling its secret when asked) and reloads. */
async function applySave(roll: boolean, ctx: ActionContext) {
  const body = { url: ctx.url.trim(), types: ctx.types, roll };
  const saved = await api<SavedHook>('/control/webhook', { key: ctx.apiKey, method: 'PUT', body });
  if (saved.secret) ctx.setSecret(saved.secret);
  ctx.toast(roll ? 'Signing secret rotated' : 'Webhook saved', 'success');
  await ctx.load();
}

/** Disables the endpoint, hides any shown secret, and reloads. */
async function applyDisable(ctx: ActionContext) {
  await api('/control/webhook', { key: ctx.apiKey, method: 'DELETE' });
  ctx.setSecret('');
  ctx.toast('Webhook disabled', 'success');
  await ctx.load();
}

/** Queues one delivery again. Not a busy action: the rest of the panel stays usable. */
async function replay(id: string, ctx: ActionContext) {
  try {
    await api(`/control/deliveries/${encodeURIComponent(id)}/replay`, { key: ctx.apiKey, method: 'POST' });
    ctx.toast('Delivery queued', 'success');
    await ctx.load();
  } catch (err) {
    ctx.setError(errorMessage(err));
  }
}

/** Save, disable and replay, bound to the panel. */
function webhookActions(ctx: ActionContext, setBusy: (busy: boolean) => void) {
  const onError = (err: unknown) => ctx.setError(errorMessage(err));
  const run = (task: () => Promise<void>) => (ctx.setError(''), withBusy(setBusy, task, onError));
  return {
    save: (roll = false) => run(() => applySave(roll, ctx)),
    disable: () => run(() => applyDisable(ctx)),
    replay: (id: string) => replay(id, ctx),
  };
}

/** Adds an event type, or removes it when already chosen. */
export function toggled(types: string[], type: string): string[] {
  return types.includes(type) ? types.filter((x) => x !== type) : [...types, type];
}

/** Everything the webhook panel renders from. */
export function useWebhook(apiKey: string) {
  const data = useWebhookData(apiKey);
  const [secret, setSecret] = useState('');
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  const toggle = (type: string) => data.setTypes((t) => toggled(t, type));
  return { ...data, secret, busy, toggle, ...webhookActions({ ...data, apiKey, toast, setSecret }, setBusy) };
}

/** The webhook panel's state. */
export type WebhookState = ReturnType<typeof useWebhook>;
