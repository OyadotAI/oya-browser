/**
 * Unit tests for the Slack message built for one event: its headline, detail,
 * context line and the live-browser and resume buttons.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { blocksFor } from '../../../../src/modules/slack/message.ts';
import { MAX_BODY_CHARS } from '../../../../src/modules/slack/constants.ts';

const LIVE = 'https://console.example.com/live/brw-1#t=tok';

/** The block of one type, if any. */
const blockOf = (message, type) => message.blocks.find((b) => b.type === type);

describe('blocksFor', () => {
  it('names the reason a run needs a person in the headline', () => {
    const message = blocksFor({ type: 'run.needs_attention', detail: { reason: 'captcha' } }, LIVE);
    assert.equal(message.text, '🔴 Oya needs you — a CAPTCHA');
  });

  it('uses an unknown reason as written', () => {
    const message = blocksFor({ type: 'run.needs_attention', detail: { reason: 'weather' } }, LIVE);
    assert.equal(message.text, '🔴 Oya needs you — weather');
  });

  it('titles a failed run and a failed session', () => {
    assert.equal(blocksFor({ type: 'run.failed', detail: { reason: 'x' } }, null).text, '⚠️ Run failed');
    assert.equal(blocksFor({ type: 'session.failed' }, null).text, '⚠️ Browser session failed');
  });

  it('falls back to the event type as the headline of an unknown event', () => {
    assert.equal(blocksFor({ type: 'something.else' }, null).text, '⚠️ something.else');
  });

  it('shows the message, else the error, else the reason, else a placeholder as the detail', () => {
    const text = (detail) => blockOf(blocksFor({ type: 'run.failed', detail }, null), 'section').text.text;
    assert.match(text({ message: 'm', error: 'e' }), /\nm$/);
    assert.match(text({ error: 'e', reason: 'r' }), /\ne$/);
    assert.match(text({ reason: 'r' }), /\nr$/);
    assert.match(text({}), /No detail was reported\.$/);
  });

  it('caps the detail at the longest body an alert carries', () => {
    const section = blockOf(blocksFor({ type: 'run.failed', detail: { error: 'x'.repeat(5000) } }, null), 'section');
    const [, body] = section.text.text.split('\n');
    assert.equal(body.length, MAX_BODY_CHARS);
  });

  it('names the browser and the run in the context line', () => {
    const message = blocksFor({ type: 'run.failed', sessionId: 'brw-1', detail: { runId: 'run_9' } }, null);
    assert.equal(blockOf(message, 'context').elements[0].text, 'Browser `brw-1` · `run_9`');
  });

  it('leaves out the context line when neither browser nor run is known', () => {
    assert.equal(blockOf(blocksFor({ type: 'run.failed' }, null), 'context'), undefined);
  });

  it('offers the live browser and a signed resume button when a run needs a person', () => {
    const event = { type: 'run.needs_attention', detail: { runId: 'run_9', owner: 'own', reason: 'login' } };
    const [live, resume] = blockOf(blocksFor(event, LIVE), 'actions').elements;
    assert.equal(live.url, LIVE);
    assert.equal(resume.action_id, 'resume_run');
    assert.deepEqual(JSON.parse(resume.value), { runId: 'run_9', owner: 'own' });
  });

  it('offers no resume button for a failed run', () => {
    const [only, ...rest] = blockOf(
      blocksFor({ type: 'run.failed', detail: { runId: 'r' } }, LIVE),
      'actions',
    ).elements;
    assert.equal(only.url, LIVE);
    assert.equal(rest.length, 0);
  });

  it('has no buttons once the browser is gone and nothing waits', () => {
    assert.equal(blockOf(blocksFor({ type: 'run.failed', detail: {} }, undefined), 'actions'), undefined);
  });
});
