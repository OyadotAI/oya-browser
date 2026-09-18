/**
 * Unit tests for scripts/diagnostics.cjs: what redaction removes before
 * anything leaves the machine.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { redact, safeUrl } = require('../../../scripts/diagnostics.cjs');

describe('safeUrl', () => {
  it('drops credentials, query and fragment', () => {
    assert.equal(safeUrl('https://u:p@x.test/a?token=1#frag'), 'https://x.test/a');
  });

  it('replaces anything unparsable whole', () => {
    assert.equal(safeUrl('not a url'), '[redacted URL]');
  });
});

describe('redact', () => {
  it('blanks values under private keys, whatever they hold', () => {
    const out = redact({ password: 'x', headers: { a: 1 }, cookie: 'c', ok: 'fine' });
    assert.deepEqual(out, { password: '[redacted]', headers: '[redacted]', cookie: '[redacted]', ok: 'fine' });
  });

  it('cleans URLs and bearer tokens inside text', () => {
    assert.equal(
      redact('go https://u:p@x.test/a?k=1 with Bearer abc.def'),
      'go https://x.test/a with Bearer [redacted]',
    );
  });

  it('removes known secret values longer than two characters', () => {
    assert.equal(redact('pw hunter2 and ab', ['hunter2', 'ab']), 'pw [redacted] and ab');
  });

  it('caps text, array and object sizes', () => {
    assert.equal(redact('x'.repeat(5000)).length, 4000);
    assert.equal(redact(Array.from({ length: 150 }, () => 1)).length, 100);
    const wide = Object.fromEntries(Array.from({ length: 150 }, (_, i) => ['k' + i, i]));
    assert.equal(Object.keys(redact(wide)).length, 100);
  });

  it('omits what is nested too deep', () => {
    let deep = 'leaf';
    for (let i = 0; i < 10; i++) deep = { n: deep };
    let node = redact(deep);
    for (let i = 0; i < 9; i++) node = node.n;
    assert.equal(node, '[omitted]');
  });

  it('passes numbers, booleans and null through', () => {
    assert.deepEqual(redact([1, true, null]), [1, true, null]);
  });
});
