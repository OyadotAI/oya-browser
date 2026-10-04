/**
 * Unit tests for the CI dependency audit: high and critical advisories block,
 * lower ones and allowed ids do not, and an advisory reached through several
 * packages counts once.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ALLOWED, blocking } from './audit.mjs';

/** One advisory as npm audit reports it under `via`. */
const advisory = (id, severity, name = 'pkg') => ({
  source: 1,
  name,
  title: `${name} problem`,
  url: `https://github.com/advisories/${id}`,
  severity,
});

/** A report with `via` entries for one package. */
const report = (...via) => ({ vulnerabilities: { pkg: { severity: 'high', via } } });

describe('the dependency audit', () => {
  it('blocks a high or critical advisory', () => {
    const found = blocking(report(advisory('GHSA-aaaa', 'high'), advisory('GHSA-bbbb', 'critical')));
    assert.deepEqual(
      found.map((a) => a.url.split('/').pop()),
      ['GHSA-aaaa', 'GHSA-bbbb'],
    );
  });

  it('lets moderate and low advisories through', () => {
    assert.deepEqual(blocking(report(advisory('GHSA-cccc', 'moderate'), advisory('GHSA-dddd', 'low'))), []);
  });

  it('lets an allowed advisory through', () => {
    assert.deepEqual(blocking(report(advisory('GHSA-vfj7-8cjw-p6xm', 'high', 'braces'))), []);
    assert.ok(Object.hasOwn(ALLOWED, 'GHSA-vfj7-8cjw-p6xm'));
  });

  it('counts an advisory reached through several packages once, and skips the names that only point to it', () => {
    const shared = advisory('GHSA-eeee', 'high');
    const found = blocking({
      vulnerabilities: { a: { via: [shared] }, b: { via: [shared, 'a'] }, c: { via: ['a'] } },
    });
    assert.equal(found.length, 1);
  });

  it('finds nothing in a clean report', () => {
    assert.deepEqual(blocking({ vulnerabilities: {} }), []);
    assert.deepEqual(blocking({}), []);
  });
});
