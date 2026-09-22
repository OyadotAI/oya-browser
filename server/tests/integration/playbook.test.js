#!/usr/bin/env node
/**
 * Playbooks replay by stable element metadata with data as placeholders, the
 * Playwright export runs, and a run parks on a person until they respond.
 *
 * Usage: node test-playbook.js
 */

import assert from 'node:assert/strict';
import {
  matchElement,
  renderPlaywright,
  variablesOf,
  missingVariables,
  sanitizeSteps,
  templateValues,
} from '../../src/modules/playbooks/service.ts';
import { fill, redact } from '../../src/modules/agent/chat.ts';
import * as runs from '../../src/modules/playbooks/runs.ts';
import { fingerprint } from '../../src/platform/audit.ts';

const page = [
  { id: 1, type: 'link', tag: 'a', text: 'Exam or Specialty Procedure', visible: true },
  { id: 2, type: 'input', tag: 'input', text: 'Member ID', domId: 'txtMember', visible: true },
  { id: 3, type: 'option', tag: 'li', text: 'Acme Health Plan', visible: true },
  { id: 4, type: 'option', tag: 'li', text: 'Aetna', visible: true },
];

assert.equal(
  matchElement({ tag: 'input', domId: 'txtMember', text: 'renamed label' }, page).id,
  2,
  'DOM id beats text',
);
assert.equal(matchElement({ type: 'link', tag: 'a', text: 'Exam or Specialty Procedure' }, page).id, 1);
assert.equal(
  matchElement({ type: 'option', tag: 'li', text: '{{insurer}}' }, page, 'aetna').id,
  4,
  'data-driven click picks by value',
);
assert.equal(matchElement({ tag: 'button', domId: 'gone' }, page), null);
assert.equal(matchElement({}, page), null, 'an unrecorded element never matches');

// Data pass-through: the model reads placeholders, the page gets values.
const data = { memberId: 'ZZ0000000001', patient: 'John Smith', sex: 'M' };
assert.equal(redact('value="ZZ0000000001" for John Smith, sex M', data), 'value="{{memberId}}" for {{patient}}, sex M');
assert.equal(fill('{{memberId}} / {{unknown}}', data), 'ZZ0000000001 / {{unknown}}');

// Filters let a run split and reformat a value and still replay with other data.
assert.equal(fill('{{n|first}} / {{n|last}} / {{n|part:2}}', { n: 'John  Q Smith' }), 'John / Smith / Q');
assert.equal(
  fill('{{dob|date:MM/DD/YYYY}} {{dob|date:YYYY}} {{dob|date:MMM D}}', { dob: 'Jan 5, 1970' }),
  '01/05/1970 1970 Jan 5',
);
assert.equal(fill('{{dob|date:DD.MM.YY}}', { dob: '1970-01-05' }), '05.01.70');
assert.equal(fill('{{p|digits}} {{p|upper}} {{p|nope}}', { p: '(555) 010-0x' }), '5550100 (555) 010-0X (555) 010-0x');
assert.equal(fill('{{dob|date:YYYY}}', { dob: 'not a date' }), 'not a date', 'an unparseable date is typed as given');

const steps = [
  { action: 'navigate', url: 'https://example.com/"q"', start: true },
  { action: 'click', el: page[0] },
  { action: 'type', el: page[1], text: '{{memberId}}' },
  { action: 'type', el: page[1], text: 'a`b${c}{{memberId}}' },
  { action: 'click', el: { ...page[2], text: '{{insurer}}' } },
  { action: 'press_key', key: 'Enter' },
  { action: 'scroll', direction: 'down', amount: 400 },
  { action: 'type', el: page[1], text: '{{memberId|digits}}' },
  { action: 'select_option', el: { type: 'select', tag: 'select', domId: 'state' }, option: '{{state|upper}}' },
];
assert.deepEqual(variablesOf(steps), ['memberId', 'insurer', 'state']);
assert.deepEqual(missingVariables({ steps, defaults: { insurer: 'Aetna' } }, {}), ['memberId', 'state']);

const code = renderPlaywright({ name: 'portal', steps });
assert.ok(!code.includes('ZZ0000000001'), 'values stay out of the generated code');

// Run the export against a fake page to prove it parses and calls what it should.
const calls = [];
const loc = (desc) => ({
  first: () => ({
    click: async () => calls.push(['click', desc]),
    fill: async (v) => calls.push(['fill', desc, v]),
    selectOption: async (o) => calls.push(['select', desc, o.label]),
  }),
});
const fake = {
  goto: async (u) => calls.push(['goto', u]),
  getByText: loc,
  getByTestId: loc,
  getByLabel: loc,
  getByPlaceholder: loc,
  locator: loc,
  keyboard: { press: async (k) => calls.push(['press', k]) },
  mouse: { wheel: async (_, y) => calls.push(['wheel', y]) },
};
await new Function(code.replace('export default ', 'return '))()(fake, {
  memberId: 'NEW1',
  insurer: 'Aetna',
  state: 'ca',
});
assert.deepEqual(calls, [
  ['goto', 'https://example.com/"q"'],
  ['click', 'Exam or Specialty Procedure'],
  ['fill', '[id="txtMember"]', 'NEW1'],
  ['fill', '[id="txtMember"]', 'a`b${c}NEW1'],
  ['click', 'Aetna'],
  ['press', 'Enter'],
  ['wheel', 400],
  ['fill', '[id="txtMember"]', '1'],
  ['select', '[id="state"]', 'CA'],
]);

// ── A person demonstrating the task: the browser records steps, the server cleans them up ──
// (browser/scripts/analyzer.js builds these in the page, so nothing here trusts the shape)
const demoed = [
  { action: 'navigate', url: 'https://portal.example.com/login', start: true, t: 1 },
  { action: 'click', el: { type: 'input', tag: 'input', text: 'User ID', domId: 'user' }, t: 2 },
  { action: 'type', el: { type: 'input', tag: 'input', text: 'User ID', domId: 'user' }, text: 'mk@oya.ai', t: 3 },
  { action: 'type', el: { type: 'input', tag: 'input', text: 'Password', domId: 'pw' }, text: '{{password}}', t: 4 },
  { action: 'select_option', el: { type: 'select', tag: 'select', domId: 'state' }, option: 'CA', t: 5 },
  { action: 'press_key', key: 'Enter', t: 6 },
  { action: 'mouse_move', x: 10, y: 20, t: 7 }, // not replayable
  { action: 'click', el: {}, t: 8 }, // no stable handle
  { action: 'navigate', url: 'javascript:alert(1)', t: 9 }, // not a page
  { action: 'click', el: { type: 'button', tag: 'button', text: 'Sign in', evil: '<script>' }, t: 10 },
];
const clean = sanitizeSteps(demoed);
assert.deepEqual(
  clean.map((s) => s.action),
  ['navigate', 'type', 'type', 'select_option', 'press_key', 'click'],
  'the click into a field it then types in is dropped, and so is everything that cannot replay',
);
assert.equal(clean[0].start, true, 'replay starts where the person started');
assert.equal(clean[1].text, 'mk@oya.ai');
assert.equal(clean[2].text, '{{password}}', 'the password was masked in the page and stays masked');
assert.equal(clean[5].el.evil, undefined, 'unknown element fields are dropped, not stored');
assert.equal(
  clean.find((s) => s.t !== undefined),
  undefined,
  'capture timestamps do not become part of the playbook',
);
assert.throws(() => sanitizeSteps('nope'), /must be an array/);
assert.throws(() => sanitizeSteps(new Array(501).fill({ action: 'press_key', key: 'Enter' })), /limited to 500/);
assert.throws(() => sanitizeSteps([{ action: 'navigate', url: 'https://x.test' }]), /no actions/);

const demoedCode = renderPlaywright({ name: 'portal-login', steps: clean });
assert.ok(!demoedCode.includes('mouse'), 'nothing unreplayable reaches the code');
const recCalls = [];
const recLoc = (desc) => ({
  first: () => ({
    click: async () => recCalls.push(['click', desc]),
    fill: async (v) => recCalls.push(['fill', desc, v]),
    selectOption: async (o) => recCalls.push(['select', desc, o.label]),
  }),
});
await new Function(demoedCode.replace('export default ', 'return '))()(
  {
    goto: async (u) => recCalls.push(['goto', u]),
    getByText: recLoc,
    getByTestId: recLoc,
    getByLabel: recLoc,
    getByPlaceholder: recLoc,
    locator: recLoc,
    keyboard: { press: async (k) => recCalls.push(['press', k]) },
    mouse: { wheel: async (_, y) => recCalls.push(['wheel', y]) },
  },
  { password: 'hunter2' },
);
assert.deepEqual(recCalls, [
  ['goto', 'https://portal.example.com/login'],
  ['fill', '[id="user"]', 'mk@oya.ai'],
  ['fill', '[id="pw"]', 'hunter2'],
  ['select', '[id="state"]', 'CA'],
  ['press', 'Enter'],
  ['click', 'Sign in'],
]);
assert.ok(!demoedCode.includes('hunter2'), 'the demoed password is never in the code');

// ── Every value becomes a field ──
// What a person typed or picked is an input with the recorded value as its
// default, so a replay that passes nothing still does what was demonstrated.
const templated = templateValues({
  prompt: 'Log in as mk@oya.ai and pick a state',
  steps: structuredClone(clean),
  defaults: {},
});
assert.deepEqual(variablesOf(templated.steps), ['user', 'password', 'state']);
assert.deepEqual(
  templated.defaults,
  { user: 'mk@oya.ai', state: 'CA' },
  'the password has no default: it never left the page',
);
assert.equal(templated.steps[0].url, 'https://portal.example.com/login', 'the addresses are the flow, not its data');
assert.equal(
  templated.prompt,
  'Log in as {{user}} and pick a state',
  'the healing agent reads the prompt, so the data in it travels too, but not a button label',
);
assert.deepEqual(missingVariables(templated, {}), ['password'], 'only the secret has to be supplied');
assert.deepEqual(templated.labels, [], 'click labels remain fixed');
assert.equal(templated.steps.at(-1).el.text, 'Sign in');

// The export of a templated recording: hand it the defaults and it repeats the run.
const fieldsCode = renderPlaywright({ ...templated, name: 'portal-login' });
assert.ok(fieldsCode.includes('mk@oya.ai'), 'non-secret defaults are included in the export');
assert.ok(
  !renderPlaywright({ ...templated, secrets: ['user'] }).includes('mk@oya.ai'),
  'secret defaults are excluded defensively',
);
const fieldCalls = [];
const fieldLoc = (desc) => ({
  first: () => ({
    click: async () => fieldCalls.push(['click', desc]),
    fill: async (v) => fieldCalls.push(['fill', desc, v]),
    selectOption: async (o) => fieldCalls.push(['select', desc, o.label]),
  }),
});
const fieldPage = {
  goto: async (u) => fieldCalls.push(['goto', u]),
  getByText: fieldLoc,
  getByTestId: fieldLoc,
  getByLabel: fieldLoc,
  getByPlaceholder: fieldLoc,
  locator: fieldLoc,
  keyboard: { press: async (k) => fieldCalls.push(['press', k]) },
  mouse: { wheel: async (_, y) => fieldCalls.push(['wheel', y]) },
};
const runExport = (vars) => new Function(fieldsCode.replace('export default ', 'return '))()(fieldPage, vars);
await runExport({ password: 'hunter2' });
assert.deepEqual(fieldCalls, recCalls, 'the defaults replay exactly what was demonstrated');

fieldCalls.length = 0;
await runExport({ user: 'ada@oya.ai', state: 'NY', password: 'hunter2' });
assert.deepEqual(
  fieldCalls,
  [
    ['goto', 'https://portal.example.com/login'],
    ['fill', '[id="user"]', 'ada@oya.ai'],
    ['fill', '[id="pw"]', 'hunter2'],
    ['select', '[id="state"]', 'NY'],
    ['press', 'Enter'],
    ['click', 'Sign in'],
  ],
  'and any one of them can be swapped',
);

// Naming: one field typed into twice is one input; two fields are two, however they were filled.
const named = templateValues({
  prompt: '',
  defaults: {},
  steps: [
    { action: 'type', el: { domId: 'from', ariaLabel: 'From' }, text: 'Ada' },
    { action: 'type', el: { domId: 'to', ariaLabel: 'To' }, text: 'Ada' },
    { action: 'type', el: { name: 'q' }, text: 'widgets' },
    { action: 'type', el: { name: 'q' }, text: 'gadgets' },
    { action: 'type', el: { placeholder: 'Date of birth' }, text: '1984-02-11' },
    { action: 'type', el: { type: 'input', tag: 'input', ariaLabel: '2024 total' }, text: '900' },
    { action: 'type', el: { tag: 'input' }, text: 'nameless' },
    { action: 'click', el: { type: 'button', tag: 'button', testId: 'submit' } }, // an icon button
  ],
});
// Never a bare `field`: a label that cannot be an identifier is told what it names,
// and a box the page never labelled is named after what it is and where it was.
assert.deepEqual(named.defaults, {
  from: 'Ada',
  to: 'Ada',
  q: 'widgets',
  q2: 'gadgets',
  dateOfBirth: '1984-02-11',
  input2024Total: '900',
  input7: 'nameless',
});
assert.equal(named.steps[1].text, '{{to}}', 'From and To stay separate inputs, however they were filled');
assert.equal(named.steps[3].text, '{{q2}}', 'the same field, corrected, is a second input');
assert.equal(named.steps[7].el.text, undefined, 'an icon button has no text to name or match on');

// A click matches on its full precedence while the value is the recorded one, and falls
// back to the value only once the caller changes it, see runStep's click branch.
assert.equal(matchElement({ ...page[1], text: 'Member ID' }, page).id, 2, 'unchanged: DOM id still wins');
assert.equal(
  matchElement({ type: 'option', tag: 'li', text: 'Acme Health Plan' }, page, 'Aetna').id,
  4,
  'changed: the old handle belongs to another choice, so the value is the key',
);

// Human attention: the run parks, only its owner can see and answer it, then it finishes.
const run = runs.start('owner', 'b1', async ({ requestHuman }) => ({
  text: await requestHuman({ reason: 'agent', message: 'Which plan?' }),
}));
assert.equal(runs.get(fingerprint('owner'), run.id).status, 'needs_attention');
assert.equal(runs.get(fingerprint('owner'), run.id).attention.message, 'Which plan?');
assert.equal(runs.get('intruder', run.id), null);
assert.equal(runs.respond('intruder', run.id, 'x'), false);
assert.equal(runs.respond(fingerprint('owner'), run.id, 'Gold'), true);
assert.equal(runs.respond(fingerprint('owner'), run.id, 'again'), false, 'one answer per request');
await new Promise((r) => setImmediate(r));
assert.equal(runs.get(fingerprint('owner'), run.id).status, 'succeeded');
assert.deepEqual(runs.get(fingerprint('owner'), run.id).result, { text: 'Gold' });

const failing = runs.start('owner', 'b1', async () => {
  throw Object.assign(new Error('Chat token quota reached'), { status: 429 });
});
await new Promise((r) => setImmediate(r));
assert.equal(runs.get(fingerprint('owner'), failing.id).status, 'failed');
assert.equal(runs.get(fingerprint('owner'), failing.id).error, 'Chat token quota reached');
assert.equal(runs.get(fingerprint('owner'), failing.id).errorStatus, 429, 'a failed run keeps its real status');

// An hourly quota lifts when the hour turns, even if the blocked key records nothing since.
const usage = await import('../../src/platform/usage.ts');
usage.record('quota-key', 'chat_input_tokens', 2_500_000);
assert.equal(usage.current('quota-key').chat_input_tokens, 2_500_000);
const RealDate = Date;
globalThis.Date = class extends RealDate {
  constructor(...args) {
    super(...(args.length ? args : [RealDate.now() + 3600_000]));
  }
  static now() {
    return RealDate.now() + 3600_000;
  }
};
try {
  assert.equal(usage.current('quota-key').chat_input_tokens, 0, "last hour's counters no longer count");
} finally {
  globalThis.Date = RealDate;
}

// The real agent loop: data reaches the page through placeholders, never reaches the model
// (not even read back from the page), and is recorded as placeholders.
const { runChat, lastRun } = await import('../../src/modules/agent/chat.ts');
const { registry } = await import('../../src/modules/browsers/registry.ts');
const { createServer } = await import('node:http');

const taskData = { patient: 'Ada Lovelace', dob: 'Dec 10, 1815' };
const secrets = { password: 's3cret-pass' };
const turns = [
  { tool: 'analyze_page', args: {} },
  { tool: 'type', args: { element_id: 1, text: '{{patient|first}}' } },
  { tool: 'type', args: { element_id: 2, text: '{{patient|last}}' } },
  { tool: 'type', args: { element_id: 3, text: '{{dob|date:MM/DD/YYYY}}' } },
  { tool: 'select_option', args: { element_id: 4, option: 'california' } },
  { tool: 'keyboard_type', args: { text: '{{password}}' } },
  { tool: 'analyze_page', args: {} },
  { text: 'DONE: filled the form' },
];
const llmBodies = [];
const llm = createServer((req, res) => {
  let body = '';
  req.on('data', (c) => {
    body += c;
  });
  req.on('end', () => {
    llmBodies.push(body);
    const next = turns[llmBodies.length - 1];
    const message = next.text
      ? { role: 'assistant', content: next.text }
      : {
          role: 'assistant',
          content: null,
          tool_calls: [
            {
              id: `call_${llmBodies.length}`,
              type: 'function',
              function: { name: next.tool, arguments: JSON.stringify(next.args) },
              extra_content: { google: { thought_signature: `sig_${llmBodies.length}` } },
            },
          ],
        };
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ choices: [{ message }], usage: { prompt_tokens: 1, completion_tokens: 1 } }));
  });
});
await new Promise((r) => llm.listen(0, '127.0.0.1', r));
process.env.OPENAI_API_KEY = 'sk-test';
process.env.OPENAI_BASE_URL = `http://127.0.0.1:${llm.address().port}/v1`;

const typed = [];
const selectEvents = [];
// The in-page select script runs against this, exactly as evaluate_raw would run it.
const stateSelect = {
  tagName: 'SELECT',
  name: 'state',
  value: '',
  labels: [],
  getAttribute: () => null,
  options: [
    { text: 'Arizona', value: 'AZ' },
    { text: 'California', value: 'CA' },
    { text: 'North Carolina', value: 'NC' },
  ],
  dispatchEvent: (e) => selectEvents.push(e.type),
};
const fakeDocument = {
  querySelectorAll: () => [stateSelect],
  getElementById: (id) => (id === 'state' ? stateSelect : null),
};
registry.add('loop-browser', {
  ws: null,
  apiKey: 'loop-key',
  name: 'loop',
  clientType: 'cdp',
  engine: {
    send: async (action, params) => {
      if (action === 'list_tabs')
        return {
          ok: true,
          data: { tabs: [{ id: 't1', url: 'https://example.com/form', title: 'Form', active: true }] },
        };
      if (action === 'analyze') {
        return {
          ok: true,
          data: {
            markdown: `form with ${typed.map(([, t]) => `value="${t}"`).join(' ')}`,
            elements: [
              { id: 1, type: 'input', tag: 'input', text: 'First name', domId: 'fname', visible: true },
              { id: 2, type: 'input', tag: 'input', text: 'Last name', domId: 'lname', visible: true },
              { id: 3, type: 'input', tag: 'input', text: 'Date of birth', domId: 'dob', visible: true },
              { id: 4, type: 'select', tag: 'select', text: 'State', domId: 'state', visible: true },
            ],
          },
        };
      }
      if (action === 'evaluate_raw') {
        return {
          ok: true,
          data: {
            result: new Function('document', 'Event', `return ${params.expression}`)(
              fakeDocument,
              class {
                constructor(type) {
                  this.type = type;
                }
              },
            ),
          },
        };
      }
      if (action === 'type' || action === 'keyboard_type') typed.push([action, params.text]);
      return { ok: true, data: {} };
    },
  },
});

const chat = await runChat(
  'loop-browser',
  [{ role: 'user', content: 'Register {{patient}} born {{dob}} in California, password {{password}}.' }],
  { apiKey: 'loop-key', data: taskData, secrets },
);
llm.close();
assert.equal(chat.text, 'DONE: filled the form');
assert.deepEqual(
  typed,
  [
    ['type', 'Ada'],
    ['type', 'Lovelace'],
    ['type', '12/10/1815'],
    ['keyboard_type', 's3cret-pass'],
  ],
  'filters split and reformat; secrets are typed for real',
);
assert.equal(stateSelect.value, 'CA', 'select_option picks the option by its text');
assert.deepEqual(selectEvents, ['input', 'change']);
assert.equal(llmBodies.length, turns.length);
assert.equal(
  JSON.parse(llmBodies[1]).messages.find((m) => m.tool_calls)?.tool_calls[0].extra_content?.google?.thought_signature,
  'sig_1',
  "a tool call's thought signature goes back to the model unchanged (Gemini 3 rejects the turn without it)",
);
assert.ok(llmBodies[0].includes('Ada Lovelace'), 'data is visible to the model so it can reason about it');
assert.ok(
  llmBodies.every((b) => !b.includes('s3cret-pass')),
  'a secret never reaches the model, even read back from the page',
);
assert.ok(
  JSON.parse(llmBodies[0]).messages[0].content.startsWith('You are a web automation agent'),
  'the automation system prompt is sent',
);
const recorded = lastRun('loop-browser').steps;
assert.deepEqual(
  recorded.map((s) => s.action),
  ['navigate', 'type', 'type', 'type', 'select_option', 'keyboard_type'],
  'every replayable action is recorded, keyboard_type included: it has a replayer and an export, and leaving it out made a recording that looked complete and typed nothing',
);
assert.deepEqual(
  recorded.slice(1, 4).map((s) => s.text),
  ['{{patient|first}}', '{{patient|last}}', '{{dob|date:MM/DD/YYYY}}'],
  'steps keep placeholders and filters, never values',
);
assert.equal(recorded[4].option, 'california');
assert.equal(recorded[4].el.domId, 'state');
assert.deepEqual(lastRun('loop-browser').secrets, ['password']);

console.log('playbook: ok');
process.exit(0);
