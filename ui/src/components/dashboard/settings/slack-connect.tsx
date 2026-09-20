/**
 * Step 1 of Slack setup: connect a workspace. Two ways in — install the Oya
 * app, or paste a bot token from an app the customer owns — and both end at
 * the same place, so a deployment with no Slack app configured simply offers
 * the second one.
 */
'use client';

import { Check, Copy, Loader2, Trash2 } from 'lucide-react';
import { useToast } from '../toast';
import type { RowComponent } from './fields';
import type { SlackState } from './use-slack';
import type { SlackConfig } from './slack-types';

/** What each part of the step gets. */
interface Props {
  /** Panel state and actions. */
  slack: SlackState;
  /** The loaded settings. */
  config: SlackConfig;
  /** The dialog's row component. */
  Row: RowComponent;
}

/** The connected workspace, with a way to disconnect. */
function Connected({ slack, config }: Omit<Props, 'Row'>) {
  return (
    <div className="flex items-center gap-3">
      <div className="min-w-0 flex-1">
        <p className="text-[13px] font-medium">{config.teamName || 'Slack'}</p>
        <p className="mt-0.5 text-[12px] text-text-muted">
          {config.byo ? 'Your own bot app' : 'Oya for Slack'} · {config.botToken}
        </p>
      </div>
      <button
        type="button"
        className="btn-ghost h-9"
        onClick={slack.disconnect}
        disabled={slack.busy}
        aria-label="Disconnect Slack"
      >
        <Trash2 className="h-3.5 w-3.5" />
        Disconnect
      </button>
    </div>
  );
}

/** Paste a bot token from your own app. */
function TokenForm({ slack, Row }: Omit<Props, 'config'>) {
  return (
    <div className="space-y-4">
      <Row
        id="slack-token"
        label="Bot token"
        hint="From your app’s OAuth & Permissions page. Needs chat:write, channels:join, channels:read and groups:read."
      >
        <input
          id="slack-token"
          className="settings-input font-mono text-[12px]"
          type="password"
          autoComplete="off"
          spellCheck={false}
          placeholder="xoxb-…"
          value={slack.token}
          onChange={(e) => slack.setToken(e.target.value)}
        />
      </Row>
      <button
        type="button"
        className="btn-ghost h-9"
        disabled={slack.busy || !slack.token.trim()}
        onClick={() => void slack.save({ botToken: slack.token.trim() })}
      >
        {slack.busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}Connect
      </button>
    </div>
  );
}

/**
 * Slack refuses any redirect URI its app config does not list, and this one
 * moves with the console's address — a dev tunnel changes it each restart.
 */
function RedirectHelp({ uri }: { /** The redirect URI. */ uri: string }) {
  const toast = useToast();
  return (
    <details className="text-[11.5px] leading-5 text-text-muted">
      <summary className="cursor-pointer hover:text-text">Slack says the redirect URI does not match?</summary>
      <p className="mt-2">
        Add this to the Slack app’s <span className="text-text">OAuth &amp; Permissions → Redirect URLs</span>:
      </p>
      <div className="mt-1.5 flex items-center gap-2">
        <code className="min-w-0 flex-1 truncate rounded-md border border-border bg-bg-sunken px-2 py-1.5 font-mono text-[11px] text-text">
          {uri}
        </code>
        <button
          type="button"
          className="btn-ghost h-8 shrink-0"
          onClick={() => void navigator.clipboard?.writeText(uri).then(() => toast('Redirect URL copied', 'success'))}
        >
          <Copy className="h-3.5 w-3.5" />
          Copy
        </button>
      </div>
    </details>
  );
}

/**
 * With an app configured the token is the secondary path, folded away behind
 * the one-click button. Without one it is the only path, so it is not an
 * "instead" of anything — a details element there renders as an inert line of
 * text above a field, which reads like a mislabelled form.
 */
function NotConnected({ slack, config, Row }: Props) {
  if (!config.oauthAvailable) {
    return (
      <div className="space-y-4">
        <p className="text-[12px] leading-5 text-text-muted">
          This deployment has no Slack app of its own, so connect one you control: create an app at{' '}
          <span className="text-text">api.slack.com/apps</span>, give it the scopes below, install it to your workspace,
          and paste its bot token.
        </p>
        <TokenForm slack={slack} Row={Row} />
      </div>
    );
  }
  return (
    <div className="space-y-4">
      <button
        type="button"
        onClick={slack.install}
        disabled={slack.busy}
        className="btn-primary h-11 w-full justify-center sm:w-auto sm:px-6"
      >
        Connect Slack
      </button>
      {config.redirectUri && <RedirectHelp uri={config.redirectUri} />}
      <details className="border-t border-border pt-4">
        <summary className="cursor-pointer text-[12px] font-medium text-text-muted hover:text-text">
          Use your own Slack app instead
        </summary>
        <div className="mt-4">
          <TokenForm slack={slack} Row={Row} />
        </div>
      </details>
    </div>
  );
}

/** Step 1's body: the workspace, or the ways to connect one. */
export default function SlackConnect(props: Props) {
  return props.config.connected ? <Connected slack={props.slack} config={props.config} /> : <NotConnected {...props} />;
}
