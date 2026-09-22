/**
 * Unit tests for scripts/workflow/generate.cjs: the Playwright module a draft
 * becomes, checked by running it against a fake page.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { generate } = require('../../../../scripts/workflow/generate.cjs');
const { fakeBrowser } = require('../../support/playwright-fakes.cjs');

/** Imports generated code as a module. */
async function load(code) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-gen-'));
  const file = path.join(dir, 'm.mjs');
  fs.writeFileSync(file, code);
  try {
    return (await import(pathToFileURL(file).href)).default;
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

const step = (id, fields) => ({ id, ...fields });

describe('generate', () => {
  it('writes a module that runs each enabled step with its values filled', async () => {
    const { code } = generate({
      variables: { who: { default: 'ann' } },
      steps: [
        step('a', { action: 'navigate', url: 'https://x.test/{{who}}' }),
        step('b', { action: 'type', text: '{{who}}', candidates: [{ kind: 'label', value: 'Name' }] }),
        step('c', { action: 'click', enabled: false, candidates: [{ kind: 'css', value: '#skip' }] }),
        step('d', { action: 'scroll', direction: 'up', amount: 200 }),
        step('e', { action: 'press_key', key: 'Enter' }),
      ],
    });
    const run = await load(code);
    const page = fakeBrowser(['about:blank']).pages[0];
    await run(page, {}, { expect: () => ({}) });
    assert.deepEqual(page.log, [
      ['goto', 'https://x.test/ann'],
      ['fill', 'label:Name', 'ann'],
      ['wheel', 0, -200],
      ['press', 'Enter'],
    ]);
  });

  it('calls the hooks around each step and honours shouldRun', async () => {
    const { code } = generate({
      steps: [step('a', { action: 'press_key', key: 'A' }), step('b', { action: 'press_key', key: 'B' })],
    });
    const calls = [];
    const hooks = {
      expect: () => ({}),
      shouldRun: (id) => id !== 'a',
      beforeStep: async (id) => calls.push('before:' + id),
      afterStep: async (id) => calls.push('after:' + id),
    };
    await (
      await load(code)
    )(fakeBrowser(['about:blank']).pages[0], {}, hooks);
    assert.deepEqual(calls, ['before:b', 'after:b']);
  });

  it('reports a failed step to its hook and rethrows', async () => {
    const { code } = generate({ steps: [step('a', { action: 'click', candidates: [{ kind: 'css', value: '#x' }] })] });
    const page = fakeBrowser(['about:blank'], { failing: ['css:#x'] }).pages[0];
    const failed = [];
    const hooks = { expect: () => ({}), failedStep: async (id, error) => failed.push([id, error.message]) };
    await assert.rejects((await load(code))(page, {}, hooks), /click failed/);
    assert.deepEqual(failed, [['a', 'click failed on css:#x']]);
  });

  it('refuses to run without a required variable or a checkpoint hook', async () => {
    const run = await load(generate({ steps: [step('a', { action: 'navigate', url: 'https://x/{{id}}' })] }).code);
    await assert.rejects(
      run(fakeBrowser(['about:blank']).pages[0], {}, { expect: () => ({}) }),
      /Missing variable: id/,
    );
    const gate = await load(generate({ steps: [step('a', { action: 'checkpoint' })] }).code);
    await assert.rejects(gate(fakeBrowser(['about:blank']).pages[0], {}, { expect: () => ({}) }), /human checkpoint/);
  });

  it('maps each step to the line of its action and never writes secret values', () => {
    const { code, mapping } = generate({
      secrets: ['pw'],
      variables: { pw: { default: 'NEVER' } },
      steps: [step('a', { action: 'type', text: '{{pw}}', candidates: [{ kind: 'css', value: '#p' }] })],
    });
    assert.match(code.split('\n')[mapping.a - 1], /\.fill\(/);
    assert.ok(!code.includes('NEVER'));
  });

  it('refuses a draft with problems or a reserved variable', () => {
    assert.throws(() => generate({ steps: [{ action: 'click' }] }), /Pick a target/);
    assert.throws(
      () => generate({ steps: [{ action: 'navigate', url: 'https://x/{{constructor}}' }] }),
      /Reserved variable/,
    );
  });

  it('writes the same code every time for the same draft', () => {
    const draft = { id: 'd', steps: [step('a', { action: 'assert_url', expected: 'https://x' })] };
    assert.equal(generate(draft).code, generate(draft).code);
  });

  it('writes the double-click, hover, back and forward a recording can hold', async () => {
    const target = [{ kind: 'css', value: '#a' }];
    const { code } = generate({
      steps: [
        step('a', { action: 'hover', candidates: target }),
        step('b', { action: 'double_click', candidates: target }),
        step('c', { action: 'go_back' }),
        step('d', { action: 'go_forward' }),
      ],
    });
    const page = fakeBrowser(['about:blank']).pages[0];
    await (
      await load(code)
    )(page, {}, { expect: () => ({}) });
    assert.deepEqual(page.log, [['hover', 'css:#a'], ['dblclick', 'css:#a'], ['back'], ['forward']]);
  });

  it('checks a page by its origin, path and route, whatever its query', async () => {
    const { code } = generate({
      steps: [step('a', { action: 'assert_page', expected: 'https://x.test/b?s=1#/active' })],
    });
    let matches;
    const expect = () => ({ toHaveURL: async (predicate) => (matches = predicate) });
    await (
      await load(code)
    )(fakeBrowser(['about:blank']).pages[0], {}, { expect });
    assert.equal(matches(new URL('https://x.test/b?s=2#/active')), true);
    assert.equal(matches(new URL('https://x.test/b#/done')), false);
    assert.equal(matches(new URL('https://x.test/login?s=1#/active')), false);
  });

  it('acts on the first recorded target that finds exactly one element', async () => {
    const candidates = [
      { kind: 'testId', value: 'row' },
      { kind: 'css', value: '#two' },
      { kind: 'css', value: '#three' },
    ];
    const { code } = generate({ steps: [step('a', { action: 'click', candidates })] });
    const page = fakeBrowser(['about:blank']).pages[0];
    page.counts = { 'testId:row': 3, 'css:#two': 1 };
    await (
      await load(code)
    )(page, {}, { expect: () => ({}) });
    assert.deepEqual(page.log, [['click', 'css:#two']]);
  });

  it('holds a page check to the query parameters it names, and reads past a /ref= path segment', async () => {
    const expected = 'https://x.test/s/ref=sr_1_1?k=cable&s=price';
    const { code } = generate({ steps: [step('a', { action: 'assert_page', expected, params: 's' })] });
    let matches;
    const expect = () => ({ toHaveURL: async (predicate) => (matches = predicate) });
    await (
      await load(code)
    )(fakeBrowser(['about:blank']).pages[0], {}, { expect });
    assert.equal(matches(new URL('https://x.test/s/ref=sr_1_3?k=other&s=price')), true);
    assert.equal(matches(new URL('https://x.test/s?k=cable&s=relevance')), false);
  });

  it('types into a search box with suggestions key by key, since a fill never opens them', async () => {
    const candidates = [{ kind: 'role', role: 'combobox', value: 'Destination' }];
    const { code } = generate({ steps: [step('a', { action: 'type', text: 'Lisbon', candidates })] });
    const page = fakeBrowser(['about:blank']).pages[0];
    await (
      await load(code)
    )(page, {}, { expect: () => ({}) });
    assert.deepEqual(
      page.log.map((entry) => [entry[0], entry[2]]),
      [
        ['fill', ''],
        ['keys', 'Lisbon'],
      ],
    );
  });
});
