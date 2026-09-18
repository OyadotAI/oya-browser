/**
 * Unit tests for the work a background run does: replay a playbook with its
 * data and secrets, or hand a prompt to the agent (LLM stubbed at fetch) and
 * fail on its step limit or a FAILED verdict.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { runJob } = await import('../../../../src/modules/playbooks/jobs.ts');
const { scriptedBrowser, stubLlm, textReply } = await import('../../support/agent.ts');

const KEY = 'jobs-key';
const BROWSER = 'b-jobs';
let browser;

/** A job for the test browser. */
const job = (extra = {}) => ({
  key: KEY,
  browserId: BROWSER,
  pb: null,
  prompt: 'do it',
  data: {},
  secrets: {},
  autoHeal: true,
  ...extra,
});

describe('runJob', () => {
  beforeEach(() => {
    process.env.OPENAI_API_KEY = 'sk-host';
    browser = scriptedBrowser(BROWSER, KEY, (action) =>
      action === 'evaluate_raw' ? { ok: true, data: { result: { present: false } } } : { ok: true, data: {} },
    );
  });
  afterEach(() => {
    browser.disconnect();
    mock.restoreAll();
    delete process.env.OPENAI_API_KEY;
    delete process.env.CHAT_MAX_ITERATIONS;
  });

  it('replays a playbook with data and secrets as its variables, checkpointing as it goes', async () => {
    const pb = { name: 'p', steps: [{ action: 'navigate', url: 'https://a.test/{{q}}?t={{token}}' }], defaults: {} };
    const result = await runJob(job({ pb, data: { q: 'x' }, secrets: { token: 's3' } }), async () => 'done');
    assert.deepEqual(result, { steps: 1, total: 1, fellBack: false });
    assert.equal(browser.calls[0].params.url, 'https://a.test/x?t=s3');
    assert.ok(
      browser.calls.some((c) => c.action === 'evaluate_raw'),
      'the checkpoint ran after the navigation',
    );
  });

  it('does not heal a broken replay when autoHeal is off', async () => {
    const pb = { name: 'p', steps: [{ action: 'teleport' }], defaults: {} };
    await assert.rejects(
      runJob(job({ pb, autoHeal: false }), async () => 'done'),
      { status: 422 },
    );
  });

  it('answers the agent’s text for a prompt', async () => {
    stubLlm([textReply('DONE: booked')]);
    assert.deepEqual(await runJob(job(), async () => 'done'), { text: 'DONE: booked' });
  });

  it('fails a prompt whose reply says FAILED anywhere', async () => {
    stubLlm([textReply('Looked around.\n  FAILED: no slots')]);
    await assert.rejects(
      runJob(job(), async () => 'done'),
      { message: 'Looked around.\n  FAILED: no slots' },
    );
  });

  it('fails a prompt that hit the step limit', async () => {
    process.env.CHAT_MAX_ITERATIONS = '1';
    stubLlm([textReply('')]);
    await assert.rejects(
      runJob(job(), async () => 'done'),
      { message: 'The agent hit its step limit without finishing' },
    );
  });
});
