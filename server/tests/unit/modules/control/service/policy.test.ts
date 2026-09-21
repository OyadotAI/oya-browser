/**
 * Unit tests for governance policy and project settings validation: known
 * fields only, host rules bounded so the matcher cannot backtrack, and
 * retention, limits and rates in range.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { validatePolicy, validateSettings } from '../../../../../src/modules/control/service/policy.ts';

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
    for (const days of [0, 3651, 1.5, '7']) refused(() => validateSettings({ auditDays: days }), 'invalid_retention');
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
