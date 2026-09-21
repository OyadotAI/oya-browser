/**
 * The Slack alerts panel: connect a workspace, then pick the channel that gets
 * alerts. It saves itself (see settings/use-slack.ts); the parts live in
 * settings/.
 */
'use client';

import type { RowComponent } from './settings/fields';
import { ErrorNote, Loading, SectionHeading } from './settings/heading';
import { SlackStep } from './settings/constants';
import SlackChannel from './settings/slack-channel';
import SlackConnect from './settings/slack-connect';
import Step from './settings/slack-step';
import { useSlack } from './settings/use-slack';

export type { SlackConfig } from './settings/slack-types';

/** Two steps, always in view: connect the workspace, then say which channel. */
export default function SlackSection({
  apiKey,
  Row,
}: {
  /** The key whose alerts are configured. */
  apiKey: string;
  /** The dialog's row component. */
  Row: RowComponent;
}) {
  const slack = useSlack(apiKey);
  const { config, error } = slack;
  if (!config) return <Loading>Loading your Slack settings…</Loading>;
  return (
    <>
      <SectionHeading eyebrow="Alerts" title="Know the moment it stalls.">
        When a run fails or needs a person, Slack gets the message, with a link that opens the live browser so whoever
        sees it can take over.
      </SectionHeading>
      {error && <ErrorNote>{error}</ErrorNote>}
      <ol className="space-y-5">
        <Step n={SlackStep.CONNECT} title="Connect your workspace" done={config.connected}>
          <SlackConnect slack={slack} config={config} Row={Row} />
        </Step>
        <Step n={SlackStep.CHANNEL} title="Choose a channel" done={!!config.channelId} muted={!config.connected}>
          <SlackChannel config={config} slack={slack} />
        </Step>
      </ol>
    </>
  );
}
