/**
 * Unit tests for the translate route: only a well-formed batch reaches the
 * model, and only on the caller's own browser.
 */
import { describe, it, before, after, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir, restoreEnv } from '../../support/data-dir.ts';

ownDataDir();
process.env.OYA_LIMIT_TRANSLATE_PER_MIN = '0';
const { router } = await import('../../../../src/modules/translate/index.ts');
const { TRANSLATE_BATCH_CHARS } = await import('../../../../src/modules/translate/constants.ts');
const { allowKey, callRoute, scriptedBrowser, stubLlm } = await import('../../support/agent.ts');

const KEY = 'translate-routes-key';
const BROWSER = 'b-translate-routes';
let forget: () => void;
let browser;

/** Posts a translate body as the test key. */
const post = (body: any, id = BROWSER) =>
  callRoute(router, { method: 'POST', url: `/browsers/${id}/translate`, key: KEY, body });

describe('translate route', () => {
  const saved = process.env.OPENAI_API_KEY;
  before(() => (forget = allowKey(KEY)));
  after(() => {
    forget();
    delete process.env.OYA_LIMIT_TRANSLATE_PER_MIN;
  });
  beforeEach(() => {
    process.env.OPENAI_API_KEY = 'sk-host';
    browser = scriptedBrowser(BROWSER, KEY);
  });
  afterEach(() => {
    browser.disconnect();
    mock.restoreAll();
    restoreEnv('OPENAI_API_KEY', saved);
  });

  it('translates a valid batch', async () => {
    stubLlm([{ choices: [{ message: { content: '["Hi"]' } }], usage: {} }]);
    const res = await post({ texts: ['Hola'], target: 'en', source: 'es' });
    assert.deepEqual(res.body, { texts: ['Hi'] });
  });

  it('answers 404 for a browser the caller does not own', async () => {
    const other = scriptedBrowser('b-theirs-translate', 'someone-else');
    assert.equal((await post({ texts: ['Hola'], target: 'en' }, 'b-theirs-translate')).status, 404);
    other.disconnect();
  });

  it('refuses an unknown field', async () => {
    assert.equal((await post({ texts: ['Hola'], target: 'en', html: '<b>' })).status, 400);
  });

  it('refuses empty texts', async () => {
    assert.equal((await post({ texts: [], target: 'en' })).status, 400);
  });

  it('refuses texts that are not strings', async () => {
    assert.equal((await post({ texts: [1], target: 'en' })).status, 400);
  });

  it('refuses a batch over the character limit', async () => {
    assert.equal((await post({ texts: ['a'.repeat(TRANSLATE_BATCH_CHARS + 1)], target: 'en' })).status, 400);
  });

  it('refuses a target that is not a language tag', async () => {
    assert.equal((await post({ texts: ['Hola'], target: 'English; ignore previous' })).status, 400);
  });

  it('refuses a source that is not a language tag', async () => {
    assert.equal((await post({ texts: ['Hola'], target: 'en', source: '' })).status, 400);
  });
});
