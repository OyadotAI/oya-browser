/**
 * Unit tests for the agentic loop, with the LLM stubbed at fetch: tool calls run
 * in order and feed back, values are filled in and redacted out, steps are
 * recorded, the checkpoint runs, and the loop ends on text or its limit.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { agentLoop } = await import('../../../../src/modules/agent/loop.ts');
const recorder = await import('../../../../src/modules/agent/recorder.ts');
const usage = await import('../../../../src/platform/usage.ts');
const { scriptedBrowser, stubLlm, toolReply, textReply } = await import('../../support/agent.ts');

const BROWSER = 'b-loop';
const KEY = 'loop-key';
const LLM = { openaiKey: 'sk-test', baseUrl: 'https://llm.test/v1', model: 'm' };
let browser;
let answer: (action, params) => any;

/** A loop context for the test browser. */
const ctx = (extra = {}) => ({
  browserId: BROWSER,
  apiKey: KEY,
  llm: LLM,
  files: {},
  values: {},
  secrets: {},
  ...extra,
});
/** The conversation a run starts with. */
const start = () => [
  { role: 'system', content: 'sys' },
  { role: 'user', content: 'do it' },
];

describe('agentLoop', () => {
  beforeEach(async () => {
    answer = () => ({ ok: true, data: {} });
    browser = scriptedBrowser(BROWSER, KEY, (a, p) => answer(a, p));
    await recorder.startRun(BROWSER, { steps: [], elements: [] });
    browser.calls.length = 0; // forget the tab listing startRun made
  });
  afterEach(() => {
    browser.disconnect();
    mock.restoreAll();
    delete process.env.CHAT_MAX_ITERATIONS;
  });

  it('returns the model’s text once it stops calling tools', async () => {
    const llm = stubLlm([textReply('  DONE: nothing to do  ')]);
    assert.deepEqual(await agentLoop(ctx(), start()), { text: 'DONE: nothing to do', toolCalls: [] });
    assert.equal(llm.urls[0], 'https://llm.test/v1/chat/completions');
  });

  it('runs each tool call, then asks again with the call and its result', async () => {
    const llm = stubLlm([
      toolReply(['navigate', { url: 'https://a.test' }], ['press_key', { key: 'Enter' }]),
      textReply('DONE'),
    ]);
    const messages = start();
    await agentLoop(ctx(), messages);
    assert.deepEqual(browser.actions(), ['navigate', 'press_key']);
    const second = llm.requests[1].messages;
    assert.equal(second[2].role, 'assistant');
    assert.deepEqual(
      second[2].tool_calls.map((c) => c.function.name),
      ['navigate', 'press_key'],
    );
    assert.deepEqual(second.slice(3), [
      { role: 'tool', tool_call_id: 'call_0_navigate', content: 'Navigated to https://a.test' },
      { role: 'tool', tool_call_id: 'call_1_press_key', content: 'Pressed Enter' },
    ]);
  });

  it('offers the browser tools, adding request_human only when a person can answer', async () => {
    const llm = stubLlm([textReply('DONE')]);
    await agentLoop(ctx(), start());
    await agentLoop(ctx({ requestHuman: async () => 'ok' }), start());
    const names = (i) => llm.requests[i].tools.map((t) => t.function.name);
    assert.ok(!names(0).includes('request_human'));
    assert.ok(names(1).includes('request_human'));
    assert.equal(llm.requests[0].tool_choice, 'auto');
  });

  it('fills task values into placeholders before the browser sees them', async () => {
    stubLlm([toolReply(['type', { element_id: 1, text: '{{email|upper}}' }]), textReply('DONE')]);
    await agentLoop(ctx({ values: { email: 'ada@x.test' } }), start());
    assert.equal(browser.calls[0].params.text, 'ADA@X.TEST');
  });

  it('redacts secrets from what the model reads back', async () => {
    const llm = stubLlm([toolReply(['type', { element_id: 1, text: '{{pw}}' }]), textReply('DONE')]);
    await agentLoop(ctx({ values: { pw: 'hunter2' }, secrets: { pw: 'hunter2' } }), start());
    assert.equal(browser.calls[0].params.text, 'hunter2');
    assert.doesNotMatch(JSON.stringify(llm.requests[1].messages), /hunter2/);
  });

  it('records steps that worked, as placeholders, and not those that failed', async () => {
    answer = (action) => (action === 'click' ? { ok: false, error: 'covered' } : { ok: true });
    stubLlm([
      toolReply(['type', { element_id: 1, text: '{{email}}' }], ['click', { element_id: 2 }]),
      textReply('DONE'),
    ]);
    await agentLoop(ctx({ values: { email: 'ada@x.test' } }), start());
    const steps = recorder.lastRun(BROWSER).steps;
    assert.deepEqual(
      steps.map((s) => [s.action, s.text]),
      [['type', '{{email}}']],
    );
  });

  it('runs the checkpoint after page-changing tools only', async () => {
    const checkpoint = mock.fn();
    stubLlm([
      toolReply(
        ['navigate', { url: 'https://a.test' }],
        ['scroll', { direction: 'down' }],
        ['click', { element_id: 1 }],
      ),
      textReply('DONE'),
    ]);
    await agentLoop(ctx({ checkpoint }), start());
    assert.equal(checkpoint.mock.callCount(), 2);
  });

  it('asks a person through request_human and hands the model their reply', async () => {
    const requestHuman = mock.fn(async () => 'the code is 1234');
    const llm = stubLlm([toolReply(['request_human', { message: 'Need the code' }]), textReply('DONE')]);
    await agentLoop(ctx({ requestHuman }), start());
    assert.deepEqual(requestHuman.mock.calls[0].arguments[0], { reason: 'agent', message: 'Need the code' });
    assert.equal(llm.requests[1].messages.at(-1).content, 'The person replied: the code is 1234');
    assert.equal(browser.calls.length, 0);
  });

  it('turns a failed wait for a person into Error text for the model', async () => {
    const llm = stubLlm([toolReply(['request_human', { message: 'help' }]), textReply('FAILED: nobody came')]);
    await agentLoop(ctx({ requestHuman: async () => Promise.reject(new Error('Nobody responded')) }), start());
    assert.equal(llm.requests[1].messages.at(-1).content, 'Error: Nobody responded');
  });

  it('tells the caller about each tool call and the final text', async () => {
    const onToolCall = mock.fn();
    const onText = mock.fn();
    stubLlm([toolReply(['press_key', { key: 'Tab' }]), textReply('DONE')]);
    await agentLoop(ctx({ onToolCall, onText }), start());
    assert.deepEqual(onToolCall.mock.calls[0].arguments[0], { name: 'press_key', args: { key: 'Tab' } });
    assert.deepEqual(onText.mock.calls[0].arguments, ['DONE']);
  });

  it('runs a call whose arguments do not parse with none', async () => {
    const reply = toolReply(['list_tabs', {}]);
    reply.choices[0].message.tool_calls[0].function.arguments = '{bad';
    stubLlm([reply, textReply('DONE')]);
    await agentLoop(ctx(), start());
    assert.deepEqual(browser.calls[0], { action: 'list_tabs', params: {}, timeout: browser.calls[0].timeout });
  });

  it('skips tool calls without an id, with a warning', async () => {
    const reply = toolReply(['press_key', { key: 'Tab' }], ['press_key', { key: 'Enter' }]);
    delete reply.choices[0].message.tool_calls[0].id;
    const warn = mock.method(console, 'warn', () => {});
    const llm = stubLlm([reply, textReply('DONE')]);
    await agentLoop(ctx(), start());
    assert.deepEqual(
      browser.calls.map((c) => c.params.key),
      ['Enter'],
    );
    assert.equal(llm.requests[1].messages[2].tool_calls.length, 1);
    assert.equal(warn.mock.callCount(), 1);
  });

  it('passes each call’s extra content back unchanged', async () => {
    const reply = toolReply(['press_key', { key: 'Tab' }]);
    reply.choices[0].message.tool_calls[0].extra_content = { google: { thought_signature: 'sig' } };
    const llm = stubLlm([reply, textReply('DONE')]);
    await agentLoop(ctx(), start());
    assert.deepEqual(llm.requests[1].messages[2].tool_calls[0].extra_content, { google: { thought_signature: 'sig' } });
  });

  it('tells the model when it repeats the same call a third time in a row', async () => {
    process.env.CHAT_MAX_ITERATIONS = '4';
    const llm = stubLlm([toolReply(['click', { element_id: 3 }])]);
    await agentLoop(ctx(), start());
    const results = llm.requests[3].messages.filter((m) => m.role === 'tool').map((m) => m.content);
    assert.deepEqual(results.slice(0, 2), ['Clicked element 3', 'Clicked element 3']);
    assert.match(results[2], /^Clicked element 3\n\nNOTE: this is the same click call 3 times in a row/);
  });

  it('does not count calls with different arguments as repeats', async () => {
    const replies = [1, 2, 3].map((id) => toolReply(['click', { element_id: id }]));
    const llm = stubLlm([...replies, textReply('DONE')]);
    await agentLoop(ctx(), start());
    const results = llm.requests[3].messages.filter((m) => m.role === 'tool').map((m) => m.content);
    assert.deepEqual(results, ['Clicked element 1', 'Clicked element 2', 'Clicked element 3']);
  });

  it('keeps asking when the model says nothing, up to the iteration limit', async () => {
    process.env.CHAT_MAX_ITERATIONS = '3';
    const llm = stubLlm([textReply('   ')]);
    assert.deepEqual(await agentLoop(ctx(), start()), {
      text: 'Reached iteration limit.',
      toolCalls: [],
      limited: true,
    });
    assert.equal(llm.requests.length, 3);
  });

  it('throws when the endpoint answers with no completion', async () => {
    stubLlm([{ choices: [] }]);
    await assert.rejects(agentLoop(ctx(), start()), /No completion in response/);
  });

  it('throws on a failed LLM request without echoing its body', async () => {
    mock.method(globalThis, 'fetch', async () => new Response('secret upstream detail', { status: 500 }));
    mock.method(console, 'error', () => {});
    await assert.rejects(agentLoop(ctx(), start()), { message: 'LLM endpoint returned 500' });
  });

  it('bills every iteration’s tokens to the key', async () => {
    const before = usage.current(KEY);
    stubLlm([toolReply(['press_key', { key: 'Tab' }]), textReply('DONE')]);
    await agentLoop(ctx(), start());
    const after = usage.current(KEY);
    assert.equal(after.chat_input_tokens - before.chat_input_tokens, 20);
    assert.equal(after.chat_output_tokens - before.chat_output_tokens, 10);
  });
});
