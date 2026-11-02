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
const { inLane } = await import('../../../../src/modules/browsers/connection/lane.ts');

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

  it('keeps the supplied start URL variable when the server agent starts and restarts recording', async () => {
    stubLlm([toolReply(['restart_recording', {}]), textReply('DONE')]);
    await runChat(BROWSER, [{ role: 'user', content: 'Continue here' }], {
      apiKey: 'chat-key',
      data: { start: 'https://a.test/' },
      verify: false,
    });
    assert.deepEqual(lastRun(BROWSER).steps, [{ action: 'navigate', url: '{{start}}', start: true }]);
  });

  it('refuses a key with no LLM credential anywhere', async () => {
    delete process.env.OPENAI_API_KEY;
    await assert.rejects(
      runChat(BROWSER, [{ role: 'user', content: 'x' }], { apiKey: 'no-llm-key' }),
      /No LLM key configured/,
    );
  });

  it('turns a provider that refuses the key or model into llm_rejected, without echoing what it said', async () => {
    for (const status of [401, 404]) {
      mock.method(globalThis, 'fetch', async () => new Response('secret provider text', { status }));
      await assert.rejects(runChat(BROWSER, [{ role: 'user', content: 'x' }], { apiKey: 'bad-key' }), (err: any) => {
        assert.equal(err.status, 422);
        assert.equal(err.code, 'llm_rejected');
        assert.match(err.message, new RegExp(`refused the request \\(${status}\\)`));
        assert.doesNotMatch(err.message, /secret provider text/);
        return true;
      });
      mock.restoreAll();
    }
  });

  it('a model that keeps failing to answer is a clear 502 after the retries, not a server error', async () => {
    mock.method(globalThis, 'fetch', async () => new Response('overloaded', { status: 503 }));
    mock.method(globalThis, 'setTimeout', (fn: () => void) => (fn(), 0));
    await assert.rejects(runChat(BROWSER, [{ role: 'user', content: 'x' }], { apiKey: 'bad-key' }), (err: any) => {
      assert.deepEqual([err.status, err.code], [502, 'llm_unavailable']);
      assert.doesNotMatch(err.message, /overloaded/);
      return true;
    });
    mock.restoreAll();
  });

  it('reports provider credit failures without retrying or exposing the upstream body', async () => {
    const fetch = mock.method(
      globalThis,
      'fetch',
      async () => new Response('private provider billing details', { status: 402 }),
    );
    await assert.rejects(runChat(BROWSER, [{ role: 'user', content: 'x' }], { apiKey: 'credit-key' }), (err: any) => {
      assert.equal(err.status, 402);
      assert.equal(err.code, 'llm_payment_required');
      assert.match(err.message, /AI provider requires payment or more credits/);
      assert.doesNotMatch(err.message, /private provider billing details|Something went wrong/);
      return true;
    });
    assert.equal(fetch.mock.callCount(), 1);
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

  it('refuses a run whose project policy disallows its provider before anything is sent to it', async () => {
    const { control } = await import('../../../../src/modules/control/service.ts');
    await control().settings('hipaa-key', { llm: { allow: [] } });
    const llm = stubLlm([textReply('DONE')]);
    await assert.rejects(runChat(BROWSER, [{ role: 'user', content: 'x' }], { apiKey: 'hipaa-key' }), {
      status: 403,
      code: 'llm_not_allowed',
    });
    assert.equal(llm.requests.length, 0);
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

  it('keeps recording into the same run when the chat carries on, so the playbook has every message', async () => {
    stubLlm([toolReply(['type', { element_id: 1, text: 'shoes' }]), textReply('DONE')]);
    await runChat(BROWSER, [{ role: 'user', content: 'search shoes' }], { apiKey: 'chat-key' });
    mock.restoreAll();
    stubLlm([toolReply(['click', { element_id: 2 }]), textReply('DONE')]);
    const chat = [
      { role: 'user', content: 'search shoes' },
      { role: 'assistant', content: 'DONE' },
      { role: 'user', content: 'open the first one' },
    ];
    await runChat(BROWSER, chat, { apiKey: 'chat-key' });
    const run = lastRun(BROWSER);
    assert.deepEqual(
      run.steps.map((s) => s.action),
      ['navigate', 'type', 'click'],
    );
    assert.equal(run.prompt, 'search shoes\n\nopen the first one');
  });

  it('tells the model on a follow-up that the earlier messages are done', async () => {
    const llm = stubLlm([textReply('DONE'), textReply('DONE')]);
    await runChat(BROWSER, [{ role: 'user', content: 'a' }], { apiKey: 'chat-key' });
    const chat = [
      { role: 'user', content: 'a' },
      { role: 'assistant', content: 'DONE' },
      { role: 'user', content: 'b' },
    ];
    await runChat(BROWSER, chat, { apiKey: 'chat-key' });
    assert.doesNotMatch(llm.requests[0].messages[0].content, /FOLLOW-UP/);
    assert.match(llm.requests[1].messages[0].content, /FOLLOW-UP: .*never redo an earlier message/);
  });

  it('checks a follow-up against the newest message, so the agent is not sent back to redo the first', async () => {
    const llm = stubLlm([
      toolReply(['navigate', { url: 'https://a.test' }]),
      textReply('DONE: details entered'),
      textReply(JSON.stringify({ verdict: 'pass' })),
    ]);
    const chat = [
      { role: 'user', content: 'find flights' },
      { role: 'assistant', content: 'DONE: three flights' },
      { role: 'user', content: 'enter my details' },
    ];
    await runChat(BROWSER, chat, { apiKey: 'chat-key', verify: true });
    const asked = JSON.stringify(llm.requests.at(-1).messages[1].content);
    assert.match(asked, /EARLIER IN THE CHAT[^]*find flights[^]*TASK:\\nenter my details/);
  });

  /** A browser whose page reads `page` when analysed (then runs `onRead`), for what an earlier run read. */
  function pageSays(page: string, onRead = () => {}) {
    browser.disconnect();
    browser = scriptedBrowser(BROWSER, 'chat-key', (action) => {
      if (action === 'list_tabs') return { ok: true, data: { tabs: [{ active: true, url: 'https://a.test/' }] } };
      if (action !== 'analyze') return { ok: true };
      onRead();
      return { ok: true, data: { markdown: page, elements: [] } };
    });
  }

  /** The chat after a first message `first`, asking `next`. */
  const followUpChat = (first: string, next: string) => [
    { role: 'user', content: first },
    { role: 'assistant', content: 'Stopped.' },
    { role: 'user', content: next },
  ];

  it('what earlier runs read rides on the assistant’s side before the newest message, never in it or the system prompt', async () => {
    pageSays('Andy Bacon worked on Apple Vision Pro');
    const llm = stubLlm([toolReply(['analyze_page', {}]), textReply('FAILED: stopped'), textReply('DONE')]);
    await inLane('tab-7', async () => {
      await runChat(BROWSER, [{ role: 'user', content: 'research candidates' }], {
        apiKey: 'chat-key',
        remember: true,
      });
      await runChat(BROWSER, followUpChat('research candidates', 'what did you find?'), {
        apiKey: 'chat-key',
        remember: true,
      });
    });
    const messages = llm.requests.at(-1).messages;
    assert.doesNotMatch(messages[0].content, /Andy Bacon/, 'never in the system prompt');
    const [recall, asked] = messages.slice(-2);
    assert.deepEqual(asked, { role: 'user', content: 'what did you find?' }, 'the person’s message is their own words');
    assert.equal(recall.role, 'assistant');
    assert.match(recall.content, /^Stopped\.\n\nEarlier in this chat[^]*Andy Bacon/);
    assert.ok(
      messages.every((m) => !m.tool_calls),
      'no made-up tool call: Gemini refuses a function call without its thought signature',
    );
    assert.equal(
      lastRun(BROWSER).prompt,
      'research candidates\n\nwhat did you find?',
      'the recorded prompt is untouched',
    );
  });

  it('a secret an earlier run read comes back as its placeholder', async () => {
    pageSays('Signed in with hunter22');
    const llm = stubLlm([toolReply(['analyze_page', {}]), textReply('DONE'), textReply('DONE')]);
    const options = { apiKey: 'chat-key', remember: true, secrets: { pw: 'hunter22' } };
    await inLane('tab-8', async () => {
      await runChat(BROWSER, [{ role: 'user', content: 'sign in' }], options);
      await runChat(BROWSER, followUpChat('sign in', 'what did it say?'), options);
    });
    assert.doesNotMatch(JSON.stringify(llm.requests.at(-1).messages), /hunter22/);
    assert.match(llm.requests.at(-1).messages.at(-2).content, /Signed in with \{\{pw\}\}/);
  });

  it('a follow-up with no reply before it gets the digest as an assistant turn of its own', async () => {
    pageSays('Fawaz Hakim, Apple');
    const llm = stubLlm([toolReply(['analyze_page', {}]), textReply('DONE'), textReply('DONE')]);
    const chat = [
      { role: 'user', content: 'go' },
      { role: 'user', content: 'and?' },
    ];
    await inLane('tab-14', async () => {
      await runChat(BROWSER, [{ role: 'user', content: 'go' }], { apiKey: 'chat-key', remember: true });
      await runChat(BROWSER, chat, { apiKey: 'chat-key', remember: true });
    });
    const [first, recall, asked] = llm.requests.at(-1).messages.slice(-3);
    assert.deepEqual(first, { role: 'user', content: 'go' });
    assert.equal(recall.role, 'assistant');
    assert.match(recall.content, /^Earlier in this chat[^]*Fawaz Hakim/);
    assert.deepEqual(asked, { role: 'user', content: 'and?' });
  });

  it('a secret given only on a later message is redacted from what earlier runs read', async () => {
    pageSays('Your code is 739912');
    const llm = stubLlm([toolReply(['analyze_page', {}]), textReply('DONE'), textReply('DONE')]);
    await inLane('tab-12', async () => {
      await runChat(BROWSER, [{ role: 'user', content: 'read it' }], { apiKey: 'chat-key', remember: true });
      await runChat(BROWSER, followUpChat('read it', 'use it'), {
        apiKey: 'chat-key',
        remember: true,
        secrets: { code: '739912' },
      });
    });
    assert.doesNotMatch(JSON.stringify(llm.requests.at(-1).messages), /739912/);
  });

  it('page text cannot close the fence: each follow-up gets a fence id of its own', async () => {
    pageSays('nothing to see');
    const llm = stubLlm([toolReply(['analyze_page', {}]), textReply('DONE'), textReply('DONE'), textReply('DONE')]);
    await inLane('tab-13', async () => {
      await runChat(BROWSER, [{ role: 'user', content: 'a' }], { apiKey: 'chat-key', remember: true });
      await runChat(BROWSER, followUpChat('a', 'b'), { apiKey: 'chat-key', remember: true });
      await runChat(BROWSER, followUpChat('a', 'c'), { apiKey: 'chat-key', remember: true });
    });
    const idOf = (request) => /marked (\S+)\./.exec(request.messages.at(-2).content)?.[1];
    const [one, two] = [idOf(llm.requests.at(-2)), idOf(llm.requests.at(-1))];
    assert.ok(one && two && one !== two);
  });

  it('a run that throws keeps what it read, marked as ended on an error', async () => {
    pageSays('Imaad Remtula, visionOS');
    stubLlm([toolReply(['analyze_page', {}]), { choices: [] }]);
    await inLane('tab-9', () =>
      assert.rejects(runChat(BROWSER, [{ role: 'user', content: 'go' }], { apiKey: 'chat-key', remember: true })),
    );
    mock.restoreAll();
    const llm = stubLlm([textReply('DONE')]);
    await inLane('tab-9', () => runChat(BROWSER, followUpChat('go', 'and?'), { apiKey: 'chat-key', remember: true }));
    assert.match(llm.requests[0].messages.at(-2).content, /\(ended on an error\):[^]*Imaad Remtula/);
  });

  it('a run the person stops is marked stopped at once', async () => {
    const stop = new AbortController();
    pageSays('Yan C., Apple', () => stop.abort());
    stubLlm([toolReply(['analyze_page', {}]), textReply('DONE')]);
    await inLane('tab-10', async () => {
      const running = runChat(BROWSER, [{ role: 'user', content: 'go' }], {
        apiKey: 'chat-key',
        remember: true,
        signal: stop.signal,
      });
      await running;
    });
    mock.restoreAll();
    const llm = stubLlm([textReply('DONE')]);
    await inLane('tab-10', () => runChat(BROWSER, followUpChat('go', 'and?'), { apiKey: 'chat-key', remember: true }));
    assert.match(llm.requests[0].messages.at(-2).content, /\(stopped\)/);
  });

  it('a chat the caller did not ask to remember keeps nothing for its follow-up', async () => {
    pageSays('Gulrez K., Apple');
    const llm = stubLlm([toolReply(['analyze_page', {}]), textReply('DONE'), textReply('DONE')]);
    await inLane('tab-11', async () => {
      await runChat(BROWSER, [{ role: 'user', content: 'go' }], { apiKey: 'chat-key' });
      await runChat(BROWSER, followUpChat('go', 'and?'), { apiKey: 'chat-key' });
    });
    assert.equal(llm.requests.at(-1).messages.at(-1).content, 'and?');
  });

  it('starts a new recording when a new chat begins', async () => {
    stubLlm([toolReply(['type', { element_id: 1, text: 'shoes' }]), textReply('DONE')]);
    await runChat(BROWSER, [{ role: 'user', content: 'search shoes' }], { apiKey: 'chat-key' });
    mock.restoreAll();
    stubLlm([textReply('DONE')]);
    await runChat(BROWSER, [{ role: 'user', content: 'something else' }], { apiKey: 'chat-key' });
    assert.deepEqual(
      lastRun(BROWSER).steps.map((s) => s.action),
      ['navigate'],
    );
  });

  it('records an empty prompt when the last user message is not text', async () => {
    stubLlm([textReply('DONE')]);
    await runChat(BROWSER, [{ role: 'user', content: [{ type: 'text', text: 'x' }] }], { apiKey: 'chat-key' });
    assert.equal(lastRun(BROWSER).prompt, '');
  });
});
