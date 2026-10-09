/** Request filters follow bounded wildcard semantics without regex backtracking or guessed resource types. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { requestMatcher, nativeResourceType } from '../../../../src/main/native-network/patterns.ts';
import { NativeInterception } from '../../../../src/main/native-network/interception.ts';
/** Only native URL and category metadata participates in a filter. */
const request = (url: string, resourceType = 'xhr') => ({ url, resourceType }) as any;
test('omitted patterns match every attributed request while an empty list matches none', () => {
  assert.equal(requestMatcher(undefined)(request('https://a.test/a')), true);
  assert.equal(requestMatcher([])(request('https://a.test/a')), false);
  assert.equal(requestMatcher([])(request('a'.repeat(40000))), false);
});
test('wildcards are anchored and question marks match exactly one character', () => {
  const match = requestMatcher([{ urlPattern: 'https://*.test/item?' }]);
  assert.equal(match(request('https://a.test/item1')), true);
  for (const url of ['https://a.test/item', 'https://a.test/item12', 'prefixhttps://a.test/item1'])
    assert.equal(match(request(url)), false);
});
test('backslash escapes operators and regex metacharacters are always literal', () => {
  const match = requestMatcher([{ urlPattern: 'https://a.test/a\\*b\\?c[0]+.$' }]);
  assert.equal(match(request('https://a.test/a*b?c[0]+.$')), true);
  assert.equal(match(request('https://a.test/axbxc0xxx')), false);
});
test('rules form a union and category restrictions use actual native metadata', () => {
  const match = requestMatcher([
    { urlPattern: '*/a', resourceType: 'Image' },
    { urlPattern: '*/b', resourceType: 'Document' },
  ]);
  assert.equal(match(request('https://a.test/a', 'image')), true);
  assert.equal(match(request('https://a.test/a', 'xhr')), false);
  assert.equal(match(request('https://a.test/b', 'subFrame')), true);
  assert.equal(nativeResourceType('cspReport'), 'CSPViolationReport');
});
test('unsupported phases, guessed Fetch distinction and malformed patterns are rejected', () => {
  for (const value of [
    null,
    {},
    [null],
    [{ urlPattern: null }],
    [{ urlPattern: 'x\\' }],
    [{ requestStage: 'Response' }],
    [{ resourceType: 'Fetch' }],
    [{ extra: true }],
    Array(33).fill({}),
    [{ urlPattern: 'a'.repeat(257) }],
  ])
    assert.throws(() => requestMatcher(value));
  assert.throws(() => new NativeInterception(() => {}, { handleAuthRequests: true }), /unavailable/);
});
test('adversarial wildcard expansion exhausts a bounded shared budget rather than hanging', () => {
  const match = requestMatcher([{ urlPattern: '*a'.repeat(120) + 'b' }]);
  assert.throws(() => match(request('a'.repeat(3000))), /work limit/);
  assert.throws(() => requestMatcher(undefined)(request('a'.repeat(40000))), /URL filter limit/);
});
test('failed policy replacement leaves the previously compiled rule intact', () => {
  const policy = new NativeInterception(() => {}, { patterns: [{ urlPattern: '*/allowed' }] });
  assert.throws(() => policy.update({ patterns: [{ requestStage: 'Response' }] }), /Request stage/);
  assert.equal(policy.accepts(request('https://a.test/allowed')), true);
  assert.equal(policy.accepts(request('https://a.test/other')), false);
});
