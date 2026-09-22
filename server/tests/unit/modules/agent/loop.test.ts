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
const { WRAP_UP_STEPS } = await import('../../../../src/modules/agent/constants.ts');

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
    assert.deepEqual(await agentLoop(ctx(), start()), { text: 'DONE: nothing to do', toolCalls: [], failed: false });
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

  it('shows a screenshot to the model as an image after the tool results, keeping only the latest', async () => {
    answer = (action, params) => ({ ok: true, data: { screenshot: `data:image/${params.format};base64,SHOT` } });
    const llm = stubLlm([toolReply(['screenshot', {}]), toolReply(['screenshot', {}]), textReply('DONE')]);
    await agentLoop(ctx(), start());
    const third = llm.requests[2].messages;
    const images = third.filter((m) => Array.isArray(m.content));
    assert.equal(images.length, 1);
    assert.equal(third.at(-1), images[0]);
    assert.deepEqual(images[0].content[1], { type: 'image_url', image_url: { url: 'data:image/jpeg;base64,SHOT' } });
    assert.ok(third.some((m) => typeof m.content === 'string' && /earlier screenshot, removed/.test(m.content)));
  });

  it('withholds screenshots from a task with secrets, since an image cannot be redacted', async () => {
    const llm = stubLlm([toolReply(['screenshot', {}]), textReply('DONE')]);
    await agentLoop(ctx({ secrets: { pw: 'hunter22' } }), start());
    const second = llm.requests[1].messages;
    assert.deepEqual(browser.actions(), []);
    assert.match(second.at(-1).content, /Screenshot withheld/);
  });

  it('always offers request_human, and the challenge tools only when the run was given them', async () => {
    const llm = stubLlm([textReply('DONE')]);
    await agentLoop(ctx(), start());
    const challenges = {
      captcha: async () => null,
      signIn: async () => null,
      mfa: async () => null,
      liveViewUrl: '/x',
    };
    await agentLoop(ctx({ challenges }), start());
    const names = (i) => llm.requests[i].tools.map((t) => t.function.name);
    assert.ok(names(0).includes('request_human') && !names(0).includes('solve_captcha'));
    assert.ok(['solve_captcha', 'sign_in', 'complete_mfa'].every((n) => names(1).includes(n)));
    assert.equal(llm.requests[0].tool_choice, 'auto');
  });

  it('refuses run_script on a task carrying secrets, without asking the browser', async () => {
    stubLlm([toolReply(['run_script', { script: 'return 1' }]), textReply('DONE')]);
    await agentLoop(ctx({ secrets: { pw: 'hunter22' } }), start());
    assert.deepEqual(browser.actions(), []);
  });

  it('tells the agent to wrap up a few steps before the limit', async () => {
    process.env.CHAT_MAX_ITERATIONS = String(WRAP_UP_STEPS + 1);
    const llm = stubLlm(Array.from({ length: WRAP_UP_STEPS + 1 }, () => toolReply(['screenshot', {}])));
    const result = await agentLoop(ctx(), start());
    assert.equal(result.limited, true);
    const nudges = llm.requests
      .at(-1)
      .messages.filter((m) => m.role === 'user' && /steps left in this run/.test(m.content));
    assert.equal(nudges.length, 1, 'said once, at the right step');
    assert.match(nudges[0].content, /DONE:|FAILED:/);
  });

  it('offers return_data only with a schema, and ends the run on the data it returns', async () => {
    const schema = { type: 'object', properties: { price: { type: 'string' } } };
    const llm = stubLlm([
      textReply('DONE'),
      toolReply(['return_data', { data: { price: '$5' } }]),
      textReply('never read'),
    ]);
    await agentLoop(ctx(), start());
    const result = await agentLoop(ctx({ schema }), start());
    const names = (i) => llm.requests[i].tools.map((t) => t.function.name);
    assert.ok(!names(0).includes('return_data'));
    const offered = llm.requests[1].tools.find((t) => t.function.name === 'return_data');
    assert.deepEqual(offered.function.parameters.properties.data, schema);
    assert.deepEqual([result.data, result.text, result.failed], [{ price: '$5' }, 'DONE: {"price":"$5"}', false]);
  });

  it('ends a run whose question no one can answer mid-run with that question, for the user to answer next', async () => {
    stubLlm([toolReply(['request_human', { message: 'Which account should I use?' }]), textReply('never read')]);
    const result = await agentLoop(ctx(), start());
    assert.deepEqual([result.text, result.failed], ['NEEDS INPUT: Which account should I use?', false]);
  });

  it('solves a CAPTCHA with the run’s solver and says so', async () => {
    const challenges = {
      captcha: async () => ({ present: true, solved: true, method: 'solver' }),
      signIn: async () => null,
      mfa: async () => null,
      liveViewUrl: '/x',
    };
    const llm = stubLlm([toolReply(['solve_captcha', {}]), textReply('DONE')]);
    await agentLoop(ctx({ challenges }), start());
    assert.match(llm.requests[1].messages.find((m) => m.role === 'tool').content, /Solved the CAPTCHA \(solver\)/);
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

  it('tells the model when it repeats the same call with the same result a third time in a row', async () => {
    process.env.CHAT_MAX_ITERATIONS = '4';
    const llm = stubLlm([toolReply(['click', { element_id: 3 }])]);
    await agentLoop(ctx(), start());
    const results = llm.requests[3].messages.filter((m) => m.role === 'tool').map((m) => m.content);
    assert.deepEqual(results.slice(0, 2), ['Clicked element 3', 'Clicked element 3']);
    assert.match(results[2], /^Clicked element 3\n\nNOTE: this is the same call with the same result/);
  });

  it('tells the model when two calls alternate and the page never changes', async () => {
    process.env.CHAT_MAX_ITERATIONS = '5';
    const llm = stubLlm([
      toolReply(['click', { element_id: 1 }]),
      toolReply(['click', { element_id: 2 }]),
      toolReply(['click', { element_id: 1 }]),
      toolReply(['click', { element_id: 2 }]),
      textReply('DONE'),
    ]);
    await agentLoop(ctx(), start());
    const results = llm.requests[4].messages.filter((m) => m.role === 'tool').map((m) => m.content);
    assert.match(results[3], /NOTE: your last 4 calls repeat the same 2 steps/);
  });

  it('does not count a repeated call as stuck when its result changes, like scrolling a long list', async () => {
    let row = 0;
    const page = () => ({
      ok: true,
      data: { elements: [{ id: 1, type: 'link', text: `Row ${++row}`, visible: true }] },
    });
    answer = (action) => (action === 'scroll' ? page() : { ok: true, data: {} });
    process.env.CHAT_MAX_ITERATIONS = '5';
    const llm = stubLlm([toolReply(['scroll', { direction: 'down' }])]);
    await agentLoop(ctx(), start());
    const results = llm.requests[4].messages.filter((m) => m.role === 'tool').map((m) => m.content);
    assert.ok(results.every((r) => !r.includes('NOTE:')));
  });

  it('stops a run that stays stuck after being warned, as a failure', async () => {
    const llm = stubLlm([toolReply(['click', { element_id: 3 }])]);
    const result = await agentLoop(ctx(), start());
    assert.deepEqual([result.failed, /kept repeating click/.test(result.text)], [true, true]);
    assert.ok(llm.requests.length < 20, 'it does not run to the iteration limit');
  });

  it('does not count calls with different arguments as repeats', async () => {
    const replies = [1, 2, 3].map((id) => toolReply(['click', { element_id: id }]));
    const llm = stubLlm([...replies, textReply('DONE')]);
    await agentLoop(ctx(), start());
    const results = llm.requests[3].messages.filter((m) => m.role === 'tool').map((m) => m.content);
    assert.deepEqual(results, ['Clicked element 1', 'Clicked element 2', 'Clicked element 3']);
  });

  it('nudges a model that says nothing, and stops one that keeps saying nothing, as a failure', async () => {
    const llm = stubLlm([textReply('   ')]);
    const result = await agentLoop(ctx(), start());
    assert.deepEqual([result.text, result.failed], ['FAILED: the model stopped answering.', true]);
    assert.equal(llm.requests.length, 3);
    assert.match(JSON.stringify(llm.requests[1].messages.at(-1)), /You replied with nothing/);
  });

  it('checks the token budget before every model call, not only the first', async () => {
    let checks = 0;
    const budget = () => {
      if (++checks > 2) throw Object.assign(new Error('Chat token quota reached for this hour'), { status: 429 });
    };
    stubLlm([toolReply(['navigate', { url: 'https://a.test' }])]);
    await assert.rejects(agentLoop(ctx({ budget }), start()), { status: 429 });
    assert.equal(checks, 3);
  });

  it('reports the iteration limit as a failure', async () => {
    process.env.CHAT_MAX_ITERATIONS = '2';
    let n = 0;
    answer = () => ({ ok: true, data: { message: `step ${++n}` } });
    stubLlm([toolReply(['scroll', { direction: 'down' }])]);
    assert.deepEqual(await agentLoop(ctx(), start()), {
      text: 'Reached iteration limit.',
      toolCalls: [],
      limited: true,
      failed: true,
    });
  });

  it('throws when the endpoint answers with no completion', async () => {
    stubLlm([{ choices: [] }]);
    await assert.rejects(agentLoop(ctx(), start()), /No completion in response/);
  });

  it('throws on a failed LLM request without echoing its body', async () => {
    mock.method(globalThis, 'fetch', async () => new Response('secret upstream detail', { status: 400 }));
    mock.method(console, 'error', () => {});
    await assert.rejects(agentLoop(ctx(), start()), { message: 'LLM endpoint returned 400' });
  });

  it('bills every iteration’s tokens to the key', async () => {
    const before = usage.current(KEY);
    stubLlm([toolReply(['press_key', { key: 'Tab' }]), textReply('DONE')]);
    await agentLoop(ctx(), start());
    const after = usage.current(KEY);
    assert.equal(after.chat_input_tokens - before.chat_input_tokens, 20);
    assert.equal(after.chat_output_tokens - before.chat_output_tokens, 10);
  });

  it('types a turn’s later calls into the element the model meant, after an earlier call renumbered the page', async () => {
    const before = [
      { id: 1, type: 'input', tag: 'input', domId: 'first', visible: true },
      { id: 2, type: 'input', tag: 'input', domId: 'second', visible: true },
    ];
    // After the first type the page is analyzed again and its elements are numbered afresh.
    const after = [
      { id: 7, type: 'input', tag: 'input', domId: 'second', visible: true },
      { id: 8, type: 'input', tag: 'input', domId: 'first', visible: true },
    ];
    recorder.setElements(BROWSER, before);
    answer = (action) => (action === 'analyze' ? { ok: true, data: { elements: after } } : { ok: true, data: {} });
    stubLlm([
      toolReply(['type', { element_id: 1, text: 'a' }], ['type', { element_id: 2, text: 'b' }]),
      textReply('DONE'),
    ]);
    await agentLoop(ctx(), start());
    const typed = browser.calls.filter((c) => c.action === 'type').map((c) => c.params.selector);
    assert.deepEqual(typed, ['[data-ac-id="1"]', '[data-ac-id="7"]']);
  });

  it('refuses a later call whose element an earlier call in the turn removed', async () => {
    recorder.setElements(BROWSER, [
      { id: 1, type: 'button', tag: 'button', domId: 'open', visible: true },
      { id: 2, type: 'button', tag: 'button', domId: 'gone', visible: true },
    ]);
    const after = [{ id: 1, type: 'button', tag: 'button', domId: 'open', visible: true }];
    answer = (action) => (action === 'analyze' ? { ok: true, data: { elements: after } } : { ok: true, data: {} });
    const llm = stubLlm([toolReply(['click', { element_id: 1 }], ['click', { element_id: 2 }]), textReply('DONE')]);
    await agentLoop(ctx(), start());
    assert.equal(browser.calls.filter((c) => c.action === 'click').length, 1);
    assert.match(JSON.stringify(llm.requests[1].messages.at(-1)), /no longer on it/);
  });

  it('records the element the model acted on, not whatever holds its id after the page re-rendered', async () => {
    recorder.setElements(BROWSER, [{ id: 1, type: 'button', tag: 'button', text: 'Save', visible: true }]);
    const after = [{ id: 1, type: 'link', tag: 'a', text: 'Help', href: '/help', visible: true }];
    answer = (action) => (action === 'analyze' ? { ok: true, data: { elements: after } } : { ok: true, data: {} });
    stubLlm([toolReply(['click', { element_id: 1 }]), textReply('DONE')]);
    await agentLoop(ctx(), start());
    const step = recorder.lastRun(BROWSER).steps.find((s) => s.action === 'click');
    assert.equal(step.el.text, 'Save');
  });

  it('keeps the plan the model writes and reads it back, without touching the page', async () => {
    const steps = [
      { step: 'Open the form', done: true },
      { step: 'Submit it', done: false },
    ];
    const llm = stubLlm([toolReply(['update_plan', { steps }]), textReply('DONE')]);
    await agentLoop(ctx(), start());
    const result = llm.requests[1].messages.find((m) => m.role === 'tool').content;
    assert.equal(result, 'Plan (1 of 2 done):\n[x] Open the form\n[ ] Submit it');
    assert.equal(browser.calls.length, 0);
  });

  it('sends a report of success back when the check finds it unsupported, then accepts the fixed one', async () => {
    const verdict = (v) => textReply(JSON.stringify(v));
    const llm = stubLlm([
      toolReply(['navigate', { url: 'https://a.test' }]),
      textReply('DONE: submitted'),
      verdict({ verdict: 'fail', reason: 'the confirmation page is not shown' }),
      toolReply(['click', { element_id: 1 }]),
      textReply('DONE: submitted, confirmation shown'),
      verdict({ verdict: 'pass' }),
    ]);
    const result = await agentLoop(ctx({ verify: true }), start());
    assert.equal(result.text, 'DONE: submitted, confirmation shown');
    assert.match(JSON.stringify(llm.requests[3].messages.at(-1)), /the confirmation page is not shown/);
  });

  it('shows the checker the page redacted, as every other model call is', async () => {
    answer = (a) =>
      a === 'analyze'
        ? { ok: true, data: { markdown: 'Card hunter22 on file', elements: [{ id: 1 }] } }
        : { ok: true, data: {} };
    const llm = stubLlm([
      toolReply(['type', { element_id: 1, text: '{{pw}}' }]),
      textReply('DONE: saved'),
      textReply(JSON.stringify({ verdict: 'pass' })),
    ]);
    recorder.setElements(BROWSER, [{ id: 1 }]);
    await agentLoop(ctx({ verify: true, secrets: { pw: 'hunter22' }, values: { pw: 'hunter22' } }), start());
    assert.doesNotMatch(JSON.stringify(llm.requests.at(-1)), /hunter22/);
  });

  it('still checks a report from a run the circling warning reset', async () => {
    const same = () => toolReply(['screenshot', {}]);
    const llm = stubLlm([
      same(),
      same(),
      same(),
      same(),
      textReply('DONE: done'),
      textReply(JSON.stringify({ verdict: 'pass' })),
    ]);
    const result = await agentLoop(ctx({ verify: true }), start());
    assert.equal(result.text, 'DONE: done');
    assert.match(JSON.stringify(llm.requests.at(-1).messages[0]), /You check a browser agent/);
  });

  it('never checks a report of failure, and gives up checking after two rounds', async () => {
    const fail = textReply(JSON.stringify({ verdict: 'fail', reason: 'no' }));
    const llm = stubLlm([
      toolReply(['navigate', { url: 'https://a.test' }]),
      textReply('DONE'),
      fail,
      textReply('DONE'),
      fail,
      textReply('DONE'),
    ]);
    const result = await agentLoop(ctx({ verify: true }), start());
    assert.equal(result.text, 'DONE');
    assert.equal(llm.requests.length, 6, 'two checks, then the report stands');
    const failing = stubLlm([toolReply(['navigate', { url: 'https://a.test' }]), textReply('FAILED: blocked')]);
    await agentLoop(ctx({ verify: true }), start());
    assert.equal(failing.requests.length, 2);
  });
});
