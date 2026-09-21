/**
 * Step 2 of Slack setup: the channel that gets alerts, saved as soon as it is
 * picked.
 */
'use client';

import { Check, Loader2, Slash } from 'lucide-react';
import { useChannelFocus, type SlackState } from './use-slack';
import type { SlackConfig } from './slack-types';

/** A note under the select: what still needs doing, or where alerts go. */
function ChannelNote({
  config,
  slack,
}: {
  /** The loaded settings. */
  config: SlackConfig;
  /** Panel state. */
  slack: SlackState;
}) {
  const selected = slack.channels?.find((c) => c.id === config.channelId);
  return (
    <>
      {!config.channelId && (
        <p className="mt-2 flex items-center gap-1.5 text-[11.5px] text-yellow">
          <Slash className="h-3 w-3" />
          Nothing is sent until you pick one.
        </p>
      )}
      {selected?.private && (
        <p className="mt-2 text-[11.5px] leading-5 text-text-muted">
          A private channel needs the bot invited: run <code className="font-mono">/invite @Oya</code> in #
          {selected.name}.
        </p>
      )}
      {config.channelId && selected && !selected.private && (
        <p className="mt-2 flex items-center gap-1.5 text-[11.5px] text-accent">
          <Check className="h-3 w-3" />
          Alerts go to #{selected.name}.
        </p>
      )}
    </>
  );
}

/** Step 2's body: the channel select, once a workspace is connected. */
export default function SlackChannel({
  config,
  slack,
}: {
  /** The loaded settings. */
  config: SlackConfig;
  /** Panel state and actions. */
  slack: SlackState;
}) {
  const channelRef = useChannelFocus(config, slack.channels);
  if (!config.connected)
    return <p className="text-[12px] leading-5 text-text-muted">Available once your workspace is connected.</p>;
  return (
    <>
      <div className="relative">
        <select
          id="slack-channel"
          aria-label="Channel for Oya alerts"
          ref={channelRef}
          className="settings-input appearance-none pr-10"
          value={config.channelId || ''}
          disabled={slack.busy || !slack.channels}
          onChange={(e) => void slack.save({ channelId: e.target.value || null })}
        >
          <option value="">{slack.channels ? 'Choose a channel…' : 'Loading channels…'}</option>
          {slack.channels?.map((c) => (
            <option key={c.id} value={c.id}>
              {c.private ? '🔒 ' : '# '}
              {c.name}
            </option>
          ))}
        </select>
        {slack.busy && (
          <Loader2 className="pointer-events-none absolute right-3.5 top-1/2 h-4 w-4 -translate-y-1/2 animate-spin text-text-muted" />
        )}
      </div>
      <ChannelNote config={config} slack={slack} />
    </>
  );
}
