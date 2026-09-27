/**
 * Unit tests for answering a free-text field while a playbook replays: the
 * answer comes from the key's model, and is billed as an agent step, with the
 * operator's model's cost booked when it ran on that.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';
import { stubFetch, json } from '../../support/http.ts';

ownDataDir('oya-answer-');
const { answerField } = await import('../../../../src/modules/playbooks/answer.ts');
const usage = await import('../../../../src/platform/usage.ts');

/** A key no other suite uses. */
const KEY = 'answer-field-key-0123456789abcdef';

describe('answerField', () => {
  const saved = process.env.OPENAI_API_KEY;
  afterEach(() => {
    mock.restoreAll();
    restoreEnv('OPENAI_API_KEY', saved);
    usage.reset();
  });

  it('answers from the model and bills one agent step, with the operator’s model’s cost', async () => {
    process.env.OPENAI_API_KEY = 'sk-host';
    stubFetch(() =>
      json({
        choices: [{ message: { content: '  A short answer.  ' } }],
        usage: { prompt_tokens: 100, completion_tokens: 10 },
      }),
    );
    assert.equal(await answerField(KEY, 'Why do you want this job?'), 'A short answer.');
    const now = usage.current(KEY);
    assert.equal(now.agent_steps, 1);
    assert.ok(now.hosted_llm_microusd > 0);
  });

  it('refuses when no model is configured', async () => {
    delete process.env.OPENAI_API_KEY;
    await assert.rejects(answerField(KEY, 'Q'), { status: 422 });
  });
});
