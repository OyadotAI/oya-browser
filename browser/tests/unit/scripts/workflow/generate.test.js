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
});
