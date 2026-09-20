/**
 * The Slack message for one event: a headline, the detail, and buttons to open
 * the live browser or resume the run.
 */
import { MAX_BODY_CHARS } from './constants.ts';

/** Headline per event type. */
const TITLES = {
  'run.needs_attention': { emoji: '🔴', text: 'Oya needs you' },
  'run.failed': { emoji: '⚠️', text: 'Run failed' },
  'session.failed': { emoji: '⚠️', text: 'Browser session failed' },
};
/** What a run parked on a person is waiting for, in words. */
const REASONS = {
  captcha: 'a CAPTCHA',
  login: 'a sign-in',
  mfa: 'an MFA prompt',
  agent: 'the agent asked for a person',
  heal_failed: 'a broken playbook step',
};

/** The message's headline, naming the reason when a run needs a person. */
function headlineOf(event, d) {
  const title = TITLES[event.type] || { emoji: '⚠️', text: event.type };
  return event.type === 'run.needs_attention' && d.reason
    ? `${title.emoji} ${title.text} — ${REASONS[d.reason] || d.reason}`
    : `${title.emoji} ${title.text}`;
}

/** The small print: which browser and which run. */
const contextOf = (event, d) =>
  [event.sessionId && `Browser \`${event.sessionId}\``, d.runId && `\`${d.runId}\``].filter(Boolean).join(' · ');

/** The headline and detail block. */
function sectionOf(headline, d) {
  const body = d.message || d.error || d.reason || 'No detail was reported.';
  return { type: 'section', text: { type: 'mrkdwn', text: `*${headline}*\n${String(body).slice(0, MAX_BODY_CHARS)}` } };
}

/** The button that opens the live browser. */
const liveButton = (liveUrl) => ({
  type: 'button',
  text: { type: 'plain_text', text: 'Open live browser ↗' },
  url: liveUrl,
  style: 'primary',
});

/** The button that resumes a run waiting on a person. */
const resumeButton = (d) => ({
  type: 'button',
  action_id: 'resume_run',
  text: { type: 'plain_text', text: '✅ Resume run' },
  // Round-tripped by Slack and returned signed, so it cannot be forged without the signing secret.
  value: JSON.stringify({ runId: d.runId, owner: d.owner }),
});

/** The buttons an event gets. */
function actionsOf(event, d, liveUrl) {
  const actions = [];
  if (liveUrl) actions.push(liveButton(liveUrl));
  if (event.type === 'run.needs_attention' && d.runId) actions.push(resumeButton(d));
  return actions;
}

/** Block Kit for one event. `liveUrl` is absent when the browser is already gone. */
export function blocksFor(event, liveUrl) {
  const d = event.detail || {};
  const headline = headlineOf(event, d);
  const context = contextOf(event, d);
  const actions = actionsOf(event, d, liveUrl);
  const blocks: any[] = [sectionOf(headline, d)];
  if (context) blocks.push({ type: 'context', elements: [{ type: 'mrkdwn', text: context }] });
  if (actions.length) blocks.push({ type: 'actions', elements: actions });
  return { text: headline, blocks };
}
