/**
 * The Slack panel's state and actions. The panel saves itself rather than
 * riding the dialog's Save: a bot token is verified against Slack before it is
 * stored, and a channel change has to reach the sink immediately for the next
 * failure to land in the right room.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import { api, errorMessage } from '@/lib/api-client';
import { useToast } from '../toast';
import { withBusy } from './busy';
import type { Channel, ChannelList, InstallLink, SlackConfig, SlackSave } from './slack-types';

/** Sets a piece of panel state. */
type Setter<T> = (value: T | ((previous: T) => T)) => void;

/** Where loaded channels go. */
type ChannelsSetter = (channels: Channel[] | null) => void;

/** Fetches the channels; a failure shows the error and leaves the list empty. */
async function fetchChannels(apiKey: string, setChannels: ChannelsSetter, setError: (e: string) => void) {
  try {
    setChannels((await api<ChannelList>('/slack/channels', { key: apiKey })).channels);
  } catch (err) {
    setError(errorMessage(err));
    setChannels([]);
  }
}

/** The channel list (null until loaded) and its loader. */
function useChannels(apiKey: string, setError: (e: string) => void) {
  const [channels, setChannels] = useState<Channel[] | null>(null);
  const load = useCallback(() => fetchChannels(apiKey, setChannels, setError), [apiKey, setError]);
  return { channels, setChannels, load };
}

/** Where the Slack settings load reports to. */
interface LoadTarget {
  /** Receives the settings. */
  setConfig: (c: SlackConfig) => void;
  /** Receives a failure. */
  setError: (e: string) => void;
  /** Loads the channels. */
  loadChannels: () => Promise<void>;
}

/** Settings arrived: show them, and fetch channels when a workspace is connected. */
function onLoaded(next: SlackConfig, target: LoadTarget) {
  target.setConfig(next);
  if (next.connected) void target.loadChannels();
}

/** Loads the Slack settings; the returned cleanup ignores an answer that arrives too late. */
function loadSlack(apiKey: string, target: LoadTarget) {
  let cancelled = false;
  api<SlackConfig>('/slack', { key: apiKey })
    .then((next) => void (!cancelled && onLoaded(next, target)))
    .catch((err) => void (!cancelled && target.setError(errorMessage(err))));
  return () => void (cancelled = true);
}

/**
 * Arriving connected with no channel, straight off the install, means step 2 is
 * the only thing left to do, so put the cursor on it rather than making them hunt.
 */
export function useChannelFocus(config: SlackConfig | null, channels: Channel[] | null) {
  const channelRef = useRef<HTMLSelectElement>(null);
  const connected = config?.connected;
  const channelId = config?.channelId;
  useEffect(() => {
    if (connected && !channelId && channels?.length) channelRef.current?.focus();
  }, [connected, channelId, channels]);
  return channelRef;
}

/** What the actions change. */
interface ActionContext {
  /** The key. */
  apiKey: string;
  /** Panel config. */
  setConfig: Setter<SlackConfig | null>;
  /** Panel error. */
  setError: (e: string) => void;
  /** The token field. */
  setToken: (t: string) => void;
  /** The channel list and its loader. */
  channels: ReturnType<typeof useChannels>;
  /** Confirmation toasts. */
  toast: ReturnType<typeof useToast>;
}

/** Stores a token or channel and merges the answer; loads channels the first time. */
async function applySave(body: SlackSave, ctx: ActionContext) {
  const next = await api<SlackConfig>('/slack', { key: ctx.apiKey, method: 'PUT', body });
  ctx.setConfig((c) => ({ ...(c as SlackConfig), ...next }));
  ctx.setToken('');
  if (!ctx.channels.channels) void ctx.channels.load();
  ctx.toast(body.channelId !== undefined ? 'Slack channel saved' : 'Slack connected', 'success');
}

/** Disconnects the workspace and forgets its channels. */
async function applyDisconnect(ctx: ActionContext) {
  ctx.setConfig(await api<SlackConfig>('/slack', { key: ctx.apiKey, method: 'DELETE' }));
  ctx.channels.setChannels(null);
  ctx.toast('Slack disconnected', 'success');
}

/** Sends the browser to Slack's install page. */
async function install(apiKey: string, setError: (e: string) => void) {
  try {
    window.location.href = (await api<InstallLink>('/slack/install?json=1', { key: apiKey })).url;
  } catch (err) {
    setError(errorMessage(err));
  }
}

/** The save / disconnect / install actions; save and disconnect clear the error and raise `busy`. */
function slackActions(ctx: ActionContext, setBusy: (busy: boolean) => void) {
  const onError = (err: unknown) => ctx.setError(errorMessage(err));
  const run = (task: () => Promise<void>) => (ctx.setError(''), withBusy(setBusy, task, onError));
  return {
    save: (body: SlackSave) => run(() => applySave(body, ctx)),
    disconnect: () => run(() => applyDisconnect(ctx)),
    install: () => install(ctx.apiKey, ctx.setError),
  };
}

/** Token field, busy flag, and the actions. */
function useSlackActions(ctx: Omit<ActionContext, 'setToken' | 'toast'>) {
  const [token, setToken] = useState('');
  const [busy, setBusy] = useState(false);
  const toast = useToast();
  return { token, setToken, busy, ...slackActions({ ...ctx, setToken, toast }, setBusy) };
}

/** Everything the Slack panel renders from. */
export function useSlack(apiKey: string) {
  const [config, setConfig] = useState<SlackConfig | null>(null);
  const [error, setError] = useState('');
  const channels = useChannels(apiKey, setError);
  useEffect(() => loadSlack(apiKey, { setConfig, setError, loadChannels: channels.load }), [apiKey, channels.load]);
  const actions = useSlackActions({ apiKey, setConfig, setError, channels });
  return { config, error, channels: channels.channels, ...actions };
}

/** The Slack panel's state. */
export type SlackState = ReturnType<typeof useSlack>;
