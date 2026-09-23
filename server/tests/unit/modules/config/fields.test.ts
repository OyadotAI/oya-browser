/**
 * Unit tests for the settings a key may hold: which are secrets, which stand in
 * for environment variables, and the closed sets that are checked.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { FIELDS, PROVIDER_CHOICES, LLM_DEFAULTS } from '../../../../src/modules/config/fields.ts';

describe('settings fields', () => {
  it('marks every credential as a secret', () => {
    const secrets = Object.entries(FIELDS)
      .filter(([, f]) => f.secret)
      .map(([name]) => name);
    assert.deepEqual(secrets.sort(), [
      'anchor_api_key',
      'browserbase_api_key',
      'browseruse_api_key',
      'captcha_api_key',
      'cdp_ws_url',
      'daytona_api_key',
      'ecs',
      'openai_api_key',
      'steel_api_key',
    ]);
  });

  it('accepts every LLM provider with defaults, and refuses others', () => {
    for (const provider of Object.keys(LLM_DEFAULTS)) assert.equal(FIELDS.llm_provider.validate(provider), provider);
    assert.throws(() => FIELDS.llm_provider.validate('mistral'), { status: 400 });
  });

  it('accepts exactly the browser providers onboarding offers', () => {
    for (const { id } of PROVIDER_CHOICES) assert.equal(FIELDS.browser_provider.validate(id), id);
    assert.throws(() => FIELDS.browser_provider.validate('selenium'), { status: 400 });
  });

  it('accepts the two CAPTCHA solvers', () => {
    assert.equal(FIELDS.captcha_solver.validate('2captcha'), '2captcha');
    assert.throws(() => FIELDS.captcha_solver.validate('manual'), /captcha_solver must be one of: capsolver, 2captcha/);
  });

  it('names only real settings as what a provider needs', () => {
    for (const p of PROVIDER_CHOICES) for (const need of p.needs) assert.ok(FIELDS[need], `${p.id} needs ${need}`);
  });

  it('gives every LLM provider a base URL and a model', () => {
    for (const d of Object.values(LLM_DEFAULTS)) assert.match(d.base, /^https:\/\//) && assert.ok(d.model);
  });
});
