/**
 * Unit tests for coherence: whether a persona's timezone is plausible for the
 * country its proxy exits in. Reported, never enforced.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { coherence } from '../../../../src/modules/proxies/coherence.ts';

const persona = { id: 'p-1' };

describe('coherence', () => {
  it('passes a timezone in a region plausible for the exit country', () => {
    assert.deepEqual(coherence(persona, { timezone: 'America/Denver' }, { geo: 'US-CO' }), {
      checked: true,
      ok: true,
      country: 'US',
      timezone: 'America/Denver',
      detail: null,
    });
  });

  it('reads the country from the start of the geo code, in any case', () => {
    assert.equal(coherence(persona, { timezone: 'Europe/Berlin' }, { geo: 'de' }).ok, true);
  });

  it('names the mismatch when the timezone contradicts the exit', () => {
    const report = coherence(persona, { timezone: 'America/New_York' }, { geo: 'DE' });
    assert.equal(report.ok, false);
    assert.equal(report.detail, 'persona p-1 reports America/New_York but exits in DE');
  });

  it('checks nothing without a proxy geo or a fingerprint timezone', () => {
    assert.deepEqual(coherence(persona, { timezone: 'UTC' }, null), { checked: false });
    assert.deepEqual(coherence(persona, null, { geo: 'US' }), { checked: false });
  });

  it('checks nothing for a country it has no regions for', () => {
    assert.deepEqual(coherence(persona, { timezone: 'Asia/Seoul' }, { geo: 'KR' }), { checked: false });
  });
});
