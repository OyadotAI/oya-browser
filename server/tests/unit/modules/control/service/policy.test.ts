/**
 * Unit tests for governance policy and project settings validation: known
 * fields only, host rules bounded so the matcher cannot backtrack, and
 * retention, limits and rates in range.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  validatePolicy,
  validateSettings,
  withAuditFloor,
  llmAllowed,
} from '../../../../../src/modules/control/service/policy.ts';
import { AUDIT_RETENTION_FLOOR_DAYS } from '../../../../../src/modules/control/service/constants.ts';

/** Asserts `fn` throws a 400 with `code`. */
const refused = (fn, code) => assert.throws(fn, { status: 400, code });

describe('validatePolicy', () => {
  it('accepts host lists, a region and the redaction flag', () => {
    assert.doesNotThrow(() =>
      validatePolicy({
        allowedHosts: ['example.com', '*.example.com', '*-aiplatform.googleapis.com', 'localhost', '10.0.0.1'],
        humanHosts: ['bank.example'],
        region: 'eu-west_1',
        redactRecording: true,
      }),
    );
  });

  it('refuses a policy that is not an object, or has unknown fields', () => {
    refused(() => validatePolicy(null), 'invalid_policy');
    refused(() => validatePolicy([]), 'invalid_policy');
    refused(() => validatePolicy({ blockedHosts: [] }), 'invalid_policy');
  });

  it('refuses an empty, oversized or non-list host list', () => {
    refused(() => validatePolicy({ allowedHosts: [] }), 'invalid_policy');
    refused(() => validatePolicy({ allowedHosts: 'example.com' }), 'invalid_policy');
    refused(() => validatePolicy({ humanHosts: Array(101).fill('a.com') }), 'invalid_policy');
  });

  it('refuses malformed host rules', () => {
    for (const rule of ['Example.com', 'http://a.com', '-a.com', 'a.com/', 'a b.com', 7])
      refused(() => validatePolicy({ allowedHosts: [rule] }), 'invalid_policy');
  });

  it('refuses a rule with more than three wildcards, which would backtrack for seconds', () => {
    assert.doesNotThrow(() => validatePolicy({ allowedHosts: ['*.a*b*c.com'] }));
    refused(() => validatePolicy({ allowedHosts: ['*.a*b*c*.com'] }), 'invalid_policy');
  });

  it('refuses a rule longer than a hostname can be', () => {
    refused(() => validatePolicy({ allowedHosts: [`${'a'.repeat(250)}.com`] }), 'invalid_policy');
  });

  it('refuses a malformed region or redaction flag', () => {
    refused(() => validatePolicy({ region: 'EU' }), 'invalid_policy');
    refused(() => validatePolicy({ region: 'x'.repeat(41) }), 'invalid_policy');
    refused(() => validatePolicy({ redactRecording: 'yes' }), 'invalid_policy');
  });
});

describe('validateSettings', () => {
  it('accepts every known setting in range, and null limits', () => {
    assert.doesNotThrow(() =>
      validateSettings({
        recordingDays: 1,
        auditDays: 3650,
        budgetUsd: 0.5,
        maxConcurrent: null,
        rates: { cdp: 0, 'oya-cloud': 1.5 },
        policy: { region: 'us' },
      }),
    );
  });

  it('refuses unknown settings and non-objects', () => {
    refused(() => validateSettings({ costUsd: 0 }), 'invalid_settings');
    refused(() => validateSettings(null), 'invalid_settings');
  });

  it('refuses retention outside 1–3650 whole days', () => {
    for (const days of [0, 3651, 1.5, '7'])
      refused(() => validateSettings({ recordingDays: days }), 'invalid_retention');
  });

  it('refuses an audit window outside 1–3650 whole days', () => {
    for (const days of [0, 3651, 1.5]) refused(() => validateSettings({ auditDays: days }), 'invalid_retention');
  });

  it('raises an audit window below the retention floor to it, so an administrator cannot shorten the record', () => {
    assert.deepEqual(withAuditFloor({ auditDays: 90, budgetUsd: 5 }), {
      auditDays: AUDIT_RETENTION_FLOOR_DAYS,
      budgetUsd: 5,
    });
    assert.deepEqual(withAuditFloor({ budgetUsd: 5 }), { budgetUsd: 5 });
  });

  it('refuses a limit that is not positive, and a fractional browser cap', () => {
    refused(() => validateSettings({ budgetUsd: 0 }), 'invalid_limit');
    refused(() => validateSettings({ budgetUsd: Infinity }), 'invalid_limit');
    refused(() => validateSettings({ maxConcurrent: 2.5 }), 'invalid_limit');
  });

  it('refuses a rate card that is not a map of non-negative prices', () => {
    refused(() => validateSettings({ rates: [] }), 'invalid_rates');
    refused(() => validateSettings({ rates: { cdp: -1 } }), 'invalid_rates');
    refused(() => validateSettings({ rates: { cdp: 'free' } }), 'invalid_rates');
  });

  it('validates a policy change as a policy', () => {
    refused(() => validateSettings({ policy: { nope: 1 } }), 'invalid_policy');
  });
});

describe('the llm setting', () => {
  it('accepts any set of known providers, an empty list and null', () => {
    for (const llm of [{ allow: ['anthropic', 'openai', 'gemini'] }, { allow: ['openai'] }, { allow: [] }, null])
      assert.doesNotThrow(() => validateSettings({ llm }));
  });

  it('refuses an unknown provider, a repeated one, extra fields or a shape that is not { allow: [] }', () => {
    for (const llm of [
      { allow: ['mistral'] },
      { allow: ['openai', 'openai'] },
      { allow: [], deny: [] },
      'none',
      ['openai'],
      {},
    ])
      refused(() => validateSettings({ llm }), 'invalid_llm_policy');
  });
});

describe('llmAllowed', () => {
  it('allows every provider when the project has no model policy', () => {
    assert.equal(llmAllowed({}, 'anthropic'), true);
    assert.equal(llmAllowed({ llm: null }, 'gemini'), true);
  });

  it('allows only the providers the policy lists', () => {
    assert.equal(llmAllowed({ llm: { allow: ['openai'] } }, 'openai'), true);
    assert.equal(llmAllowed({ llm: { allow: ['openai'] } }, 'anthropic'), false);
    assert.equal(llmAllowed({ llm: { allow: [] } }, 'openai'), false);
  });
});
