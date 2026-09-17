'use client';

import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { Check, Copy, Loader2, Slash, Trash2 } from 'lucide-react';
import { api, errorMessage } from '@/lib/api-client';
import { useToast } from './toast';

export type SlackConfig = {
  connected: boolean;
  oauthAvailable: boolean;
  redirectUri: string | null;
  teamName: string | null;
  channelId: string | null;
  channelName: string | null;
  byo: boolean;
  botToken: string;
};
type Channel = { id: string; name: string; private: boolean };

/** One numbered step, ticked once it is satisfied. */
function Step({ n, title, done, muted, children }: { n: number; title: string; done: boolean; muted?: boolean; children: ReactNode }) {
  return (
    <li className={`rounded-lg border p-4 ${done ? 'border-accent/25 bg-accent/[0.04]' : muted ? 'border-border bg-bg-sunken/40' : 'border-border bg-bg-sunken/60'}`}>
      <div className="mb-3 flex items-center gap-2.5">
        <span aria-hidden="true" className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[11px] font-semibold ${done ? 'bg-accent text-bg' : 'bg-text/10 text-text-muted'}`}>
          {done ? <Check className="h-3 w-3" /> : n}
        </span>
        <h4 className={`text-[13px] font-medium ${muted ? 'text-text-muted' : 'text-text'}`}>{title}</h4>
      </div>
      {children}
    </li>
  );
}

/**
 * Connect Slack and pick the channel that gets alerts. Two ways in — install the
 * Oya app, or paste a bot token from an app the customer owns — and both end at
 * the same place, so a deployment with no Slack app configured simply offers the
 * second one.
 *
 * This panel saves itself rather than riding the dialog's Save: a bot token is
 * verified against Slack before it is stored, and a channel change has to reach
 * the sink immediately for the next failure to land in the right room.
 */
export default function SlackSection({ apiKey, Row }: { apiKey: string; Row: (props: { id?: string; label: string; hint?: string; children: ReactNode }) => ReactNode }) {
  const toast = useToast();
  const [config, setConfig] = useState<SlackConfig | null>(null);
  const [channels, setChannels] = useState<Channel[] | null>(null);
  const [token, setToken] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const channelRef = useRef<HTMLSelectElement>(null);

  // Arriving connected with no channel — straight off the install — means step 2 is
  // the only thing left to do, so put the cursor on it rather than making them hunt.
  useEffect(() => {
    if (config?.connected && !config.channelId && channels?.length) channelRef.current?.focus();
  }, [config?.connected, config?.channelId, channels]);

  const loadChannels = useCallback(async () => {
    try { setChannels((await api<{ channels: Channel[] }>('/slack/channels', { key: apiKey })).channels); }
    catch (err) { setError(errorMessage(err)); setChannels([]); }
  }, [apiKey]);

  useEffect(() => {
    let cancelled = false;
    api<SlackConfig>('/slack', { key: apiKey })
      .then(next => { if (cancelled) return; setConfig(next); if (next.connected) void loadChannels(); })
      .catch(err => { if (!cancelled) setError(errorMessage(err)); });
    return () => { cancelled = true; };
  }, [apiKey, loadChannels]);

  const save = async (body: { botToken?: string; channelId?: string | null }) => {
    setBusy(true); setError('');
    try {
      const next = await api<SlackConfig>('/slack', { key: apiKey, method: 'PUT', body });
      setConfig(c => ({ ...(c as SlackConfig), ...next }));
      setToken('');
      if (!channels) void loadChannels();
      toast(body.channelId !== undefined ? 'Slack channel saved' : 'Slack connected', 'success');
    } catch (err) { setError(errorMessage(err)); }
    finally { setBusy(false); }
  };

  const disconnect = async () => {
    setBusy(true); setError('');
    try {
      setConfig(await api<SlackConfig>('/slack', { key: apiKey, method: 'DELETE' }));
      setChannels(null);
      toast('Slack disconnected', 'success');
    } catch (err) { setError(errorMessage(err)); }
    finally { setBusy(false); }
  };

  const install = async () => {
    try { window.location.href = (await api<{ url: string }>('/slack/install?json=1', { key: apiKey })).url; }
    catch (err) { setError(errorMessage(err)); }
  };

  if (!config) return <div className="flex min-h-[270px] items-center justify-center gap-2 text-sm text-text-muted" role="status"><Loader2 className="h-4 w-4 animate-spin" />Loading your Slack settings…</div>;

  const selected = channels?.find(c => c.id === config.channelId);
  const tokenForm = (
    <div className="space-y-4">
      <Row id="slack-token" label="Bot token" hint="From your app’s OAuth & Permissions page. Needs chat:write, channels:join, channels:read and groups:read.">
        <input id="slack-token" className="settings-input font-mono text-[12px]" type="password" autoComplete="off" spellCheck={false}
          placeholder="xoxb-…" value={token} onChange={e => setToken(e.target.value)} />
      </Row>
      <button type="button" className="btn-ghost h-9" disabled={busy || !token.trim()} onClick={() => void save({ botToken: token.trim() })}>
        {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}Connect
      </button>
    </div>
  );
  return (
    <>
      <div className="mb-7">
        <p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent">Alerts</p>
        <h3 className="text-[22px] font-semibold tracking-[-0.035em]">Know the moment it stalls.</h3>
        <p className="mt-1.5 text-[13px] leading-6 text-text-muted">When a run fails or needs a person, Slack gets the message — with a link that opens the live browser so whoever sees it can take over.</p>
      </div>
      {error && <div role="alert" className="mb-5 rounded-lg border border-red/25 bg-red/5 p-3 text-[13px] text-red">{error}</div>}

      {/* Two steps, always in view: connect the workspace, then say which channel. */}
      <ol className="space-y-5">
        <Step n={1} title="Connect your workspace" done={config.connected}>
          {config.connected ? (
            <div className="flex items-center gap-3">
              <div className="min-w-0 flex-1">
                <p className="text-[13px] font-medium">{config.teamName || 'Slack'}</p>
                <p className="mt-0.5 text-[12px] text-text-muted">{config.byo ? 'Your own bot app' : 'Oya for Slack'} · {config.botToken}</p>
              </div>
              <button type="button" className="btn-ghost h-9" onClick={disconnect} disabled={busy} aria-label="Disconnect Slack"><Trash2 className="h-3.5 w-3.5" />Disconnect</button>
            </div>
          ) : (
            <div className="space-y-4">
              {config.oauthAvailable && (
                <>
                  <button type="button" onClick={install} disabled={busy} className="btn-primary h-11 w-full justify-center sm:w-auto sm:px-6">Connect Slack</button>
                  {/* Slack refuses any redirect URI its app config does not list, and this
                      one moves with the console's address — a dev tunnel changes it each restart. */}
                  {config.redirectUri && (
                    <details className="text-[11.5px] leading-5 text-text-muted">
                      <summary className="cursor-pointer hover:text-text">Slack says the redirect URI does not match?</summary>
                      <p className="mt-2">Add this to the Slack app’s <span className="text-text">OAuth &amp; Permissions → Redirect URLs</span>:</p>
                      <div className="mt-1.5 flex items-center gap-2">
                        <code className="min-w-0 flex-1 truncate rounded-md border border-border bg-bg-sunken px-2 py-1.5 font-mono text-[11px] text-text">{config.redirectUri}</code>
                        <button type="button" className="btn-ghost h-8 shrink-0" onClick={() => void navigator.clipboard?.writeText(config.redirectUri as string).then(() => toast('Redirect URL copied', 'success'))}>
                          <Copy className="h-3.5 w-3.5" />Copy
                        </button>
                      </div>
                    </details>
                  )}
                </>
              )}
              {/* With an app configured this is the secondary path, folded away behind
                  the one-click button. Without one it is the only path, so it is not
                  an "instead" of anything — a details element there renders as an inert
                  line of text above a field, which reads like a mislabelled form. */}
              {config.oauthAvailable ? (
                <details className="border-t border-border pt-4">
                  <summary className="cursor-pointer text-[12px] font-medium text-text-muted hover:text-text">Use your own Slack app instead</summary>
                  <div className="mt-4">{tokenForm}</div>
                </details>
              ) : (
                <>
                  <p className="text-[12px] leading-5 text-text-muted">
                    This deployment has no Slack app of its own, so connect one you control:
                    create an app at <span className="text-text">api.slack.com/apps</span>, give it
                    the scopes below, install it to your workspace, and paste its bot token.
                  </p>
                  {tokenForm}
                </>
              )}
            </div>
          )}
        </Step>

        <Step n={2} title="Choose a channel" done={!!config.channelId} muted={!config.connected}>
          {config.connected ? (
            <>
              <div className="relative">
                <select id="slack-channel" aria-label="Channel for Oya alerts" ref={channelRef} className="settings-input appearance-none pr-10"
                  value={config.channelId || ''} disabled={busy || !channels}
                  onChange={e => void save({ channelId: e.target.value || null })}>
                  <option value="">{channels ? 'Choose a channel…' : 'Loading channels…'}</option>
                  {channels?.map(c => <option key={c.id} value={c.id}>{c.private ? '🔒 ' : '# '}{c.name}</option>)}
                </select>
                {busy && <Loader2 className="pointer-events-none absolute right-3.5 top-1/2 h-4 w-4 -translate-y-1/2 animate-spin text-text-muted" />}
              </div>
              {!config.channelId && <p className="mt-2 flex items-center gap-1.5 text-[11.5px] text-yellow"><Slash className="h-3 w-3" />Nothing is sent until you pick one.</p>}
              {selected?.private && <p className="mt-2 text-[11.5px] leading-5 text-text-muted">A private channel needs the bot invited: run <code className="font-mono">/invite @Oya</code> in #{selected.name}.</p>}
              {config.channelId && selected && !selected.private && <p className="mt-2 flex items-center gap-1.5 text-[11.5px] text-accent"><Check className="h-3 w-3" />Alerts go to #{selected.name}.</p>}
            </>
          ) : <p className="text-[12px] leading-5 text-text-muted">Available once your workspace is connected.</p>}
        </Step>
      </ol>
    </>
  );
}
