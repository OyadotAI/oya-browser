/**
 * Unit tests for the main-world scripts that pick a <select> option and attach
 * a file: what they send, how answers are read, and that every interpolated
 * value stays a literal.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { selectOptionIn, uploadFileIn, UPLOAD_FILE_JS } = await import('../../../../src/modules/agent/page-scripts.ts');
const { UPLOAD_TIMEOUT_MS } = await import('../../../../src/modules/agent/constants.ts');
const { scriptedBrowser } = await import('../../support/agent.ts');

const BROWSER = 'b-scripts';
const HOSTILE = '"); globalThis.pwned = true; ("';
let answer: () => any;
let browser;

describe('page scripts', () => {
  beforeEach(() => {
    answer = () => ({ ok: true, data: { result: { ok: true } } });
    browser = scriptedBrowser(BROWSER, 'key-a', () => answer());
  });
  afterEach(() => browser.disconnect());

  describe('selectOptionIn', () => {
    it('runs a main-world script and returns its result', async () => {
      answer = () => ({ ok: true, data: { result: { ok: true, chosen: 'Blue' } } });
      assert.deepEqual(await selectOptionIn(BROWSER, { name: 'color' }, 'Blue'), { ok: true, chosen: 'Blue' });
      assert.equal(browser.calls[0].action, 'evaluate_raw');
    });

    it('interpolates handles and the option only as literals', async () => {
      await selectOptionIn(BROWSER, { domId: HOSTILE, name: HOSTILE }, HOSTILE);
      const { expression } = browser.calls[0].params;
      assert.doesNotThrow(() => new Function(`return ${expression}`));
      assert.ok(expression.includes(JSON.stringify(HOSTILE)));
    });

    it('reads a driver that answers with the result itself', async () => {
      answer = () => ({ ok: true, data: { ok: true, chosen: 'X' } });
      assert.deepEqual(await selectOptionIn(BROWSER, null, 'x'), { ok: true, chosen: 'X' });
    });

    it('fails with the browser’s error, or a generic one', async () => {
      answer = () => ({ ok: false, error: 'detached' });
      assert.deepEqual(await selectOptionIn(BROWSER, {}, 'x'), { ok: false, error: 'detached' });
      answer = () => ({ ok: false });
      assert.deepEqual(await selectOptionIn(BROWSER, {}, 'x'), { ok: false, error: 'select failed' });
      answer = () => ({ ok: true });
      assert.deepEqual(await selectOptionIn(BROWSER, {}, 'x'), { ok: false, error: 'select failed' });
    });
  });

  describe('uploadFileIn', () => {
    it('sends the file in one evaluate with the upload timeout', async () => {
      answer = () => ({ ok: true, data: { result: { ok: true, field: 'cv' } } });
      const r = await uploadFileIn(BROWSER, null, { file: 'a.pdf', type: 'application/pdf', b64: 'AAAA' });
      assert.deepEqual(r, { ok: true, field: 'cv' });
      assert.equal(browser.calls[0].timeout, UPLOAD_TIMEOUT_MS);
      assert.match(browser.calls[0].params.expression, /const b64 = "AAAA";/);
    });

    it('fails with the browser’s error, or a generic one', async () => {
      answer = () => ({ ok: false });
      assert.deepEqual(await uploadFileIn(BROWSER, {}, null), { ok: false, error: 'upload failed' });
      answer = () => ({ ok: true });
      assert.deepEqual(await uploadFileIn(BROWSER, {}, {}), { ok: false, error: 'upload failed' });
    });

    it('interpolates the handle and file name only as literals', () => {
      const js = UPLOAD_FILE_JS({ text: HOSTILE }, { file: HOSTILE, type: 'x', b64: '' });
      assert.doesNotThrow(() => new Function(`return ${js}`));
      assert.ok(!js.includes(`${HOSTILE};`));
    });
  });
});
