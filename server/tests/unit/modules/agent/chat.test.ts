/**
 * Unit tests for runChat, with the LLM stubbed at fetch: the key's model
 * settings and budget, how task data, files and secrets reach the prompt, and
 * the recorded run it leaves behind.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { runChat, lastRun } = await import('../../../../src/modules/agent/chat.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');
const usage = await import('../../../../src/platform/usage.ts');
const { QUOTAS } = await import('../../../../src/platform/limits.ts');
const { scriptedBrowser, stubLlm, toolReply, textReply } = await import('../../support/agent.ts');

const BROWSER = 'b-chat';
const FILE = { file: 'cv.pdf', type: 'application/pdf', b64: 'AAAA' };
let browser;

describe('runChat', () => {
  beforeEach(() => {
    process.env.OPENAI_API_KEY = 'sk-host';
    browser = scriptedBrowser(BROWSER, 'chat-key', (action) =>
      action === 'list_tabs' ? { ok: true, data: { tabs: [{ active: true, url: 'https://a.test/' }] } } : { ok: true },
    );
  });
  afterEach(() => {
    delete process.env.OPENAI_API_KEY;
    browser.disconnect();
    mock.restoreAll();
    keyConfig.reset();
  });

  it('refuses a key with no LLM credential anywhere', async () => {
    delete process.env.OPENAI_API_KEY;
    await assert.rejects(
      runChat(BROWSER, [{ role: 'user', content: 'x' }], { apiKey: 'no-llm-key' }),
      /No LLM key configured/,
    );
  });

  it('refuses with 429 once the deployment’s hourly token budget is spent', async () => {
    usage.record('spent-key', 'chat_input_tokens', QUOTAS.chatTokensPerHour);
    await assert.rejects(runChat(BROWSER, [{ role: 'user', content: 'x' }], { apiKey: 'spent-key' }), {
      status: 429,
      message: /Chat token quota reached/,
    });
  });

  it('lets a key with its own credential past the budget, using its own endpoint', async () => {
    usage.record('own-key', 'chat_input_tokens', QUOTAS.chatTokensPerHour);
    await keyConfig.set('own-key', { openai_api_key: 'sk-own', llm_provider: 'anthropic' });
    const claudeReply = { type: 'message', role: 'assistant', content: [{ type: 'text', text: 'DONE' }], usage: {} };
    const llm = stubLlm([claudeReply]);
    const result = await runChat(BROWSER, [{ role: 'user', content: 'x' }], { apiKey: 'own-key' });
    assert.equal(result.text, 'DONE');
    assert.match(llm.urls[0], /^https:\/\/api\.anthropic\.com\/v1\/messages/);
    assert.equal(llm.requests[0].model, 'claude-opus-5');
  });

  it('tells the model its data and files, and names secrets without their values', async () => {
    const llm = stubLlm([textReply('DONE')]);
    const data = { email: 'ada@x.test', cv: FILE };
    await runChat(BROWSER, [{ role: 'user', content: 'Apply as ada@x.test with pw hunter2' }], {
      apiKey: 'chat-key',
      data,
      secrets: { pw: 'hunter2' },
    });
    const [system, user] = llm.requests[0].messages;
    assert.match(system.content, /\{\{email\}\} = "ada@x\.test"/);
    assert.match(system.content, /cv, "cv\.pdf"/);
    assert.match(system.content, /SECRETS \(hidden from you\): \{\{pw\}\}/);
    assert.doesNotMatch(JSON.stringify(llm.requests[0]), /hunter2/);
    assert.equal(user.content, 'Apply as ada@x.test with pw {{pw}}');
  });

  it('records the run from the current page, its prompt redacted to placeholders', async () => {
    stubLlm([toolReply(['type', { element_id: 1, text: '{{email}}' }]), textReply('DONE')]);
    await runChat(BROWSER, [{ role: 'user', content: 'Sign up ada@x.test' }], {
      apiKey: 'chat-key',
      data: { email: 'ada@x.test' },
      secrets: { pw: 'hunter2' },
    });
    const run = lastRun(BROWSER);
    assert.equal(run.prompt, 'Sign up {{email}}');
    assert.deepEqual(run.secrets, ['pw']);
    assert.deepEqual(
      run.steps.map((s) => s.action),
      ['navigate', 'type'],
    );
    assert.equal(browser.calls.find((c) => c.action === 'type').params.text, 'ada@x.test');
  });

  it('records an empty prompt when the last user message is not text', async () => {
    stubLlm([textReply('DONE')]);
    await runChat(BROWSER, [{ role: 'user', content: [{ type: 'text', text: 'x' }] }], { apiKey: 'chat-key' });
    assert.equal(lastRun(BROWSER).prompt, '');
  });
});
