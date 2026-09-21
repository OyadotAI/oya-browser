/**
 * Unit tests for task values in text: placeholders filled with their filters,
 * values redacted back to placeholders, and file values recognised.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { FILTERS, pipesOf, fill, redact, isFileValue, dataKey } from '../../../../src/modules/agent/placeholders.ts';

describe('filters', () => {
  it('splits a name into words', () => {
    assert.equal(FILTERS.first('  Ada King Lovelace '), 'Ada');
    assert.equal(FILTERS.last('Ada King Lovelace'), 'Lovelace');
    assert.equal(FILTERS.part('Ada King Lovelace', '2'), 'King');
    assert.equal(FILTERS.part('Ada', '5'), '');
    assert.equal(FILTERS.first('   '), '');
  });

  it('changes case and keeps only digits', () => {
    assert.equal(FILTERS.upper('ab'), 'AB');
    assert.equal(FILTERS.lower('AB'), 'ab');
    assert.equal(FILTERS.digits('+1 (555) 010-9999'), '15550109999');
  });

  it('formats an ISO date in UTC so the day never shifts', () => {
    assert.equal(FILTERS.date('2024-03-05'), '03/05/2024');
    assert.equal(FILTERS.date('2024-03-05', 'D MMMM YYYY'), '5 March 2024');
    assert.equal(FILTERS.date('2024-12-25', 'MMM D, YY'), 'Dec 25, 24');
    assert.equal(FILTERS.date('2024-03-05', 'M/DD'), '3/05');
  });

  it('formats other date strings in local time', () => {
    assert.equal(FILTERS.date('March 5, 2024 12:00', 'YYYY-MM-DD'), '2024-03-05');
  });

  it('leaves a value that is not a date as it is', () => {
    assert.equal(FILTERS.date('someday'), 'someday');
  });
});

describe('pipesOf', () => {
  it('parses filters and their arguments', () => {
    assert.deepEqual(pipesOf('|first|date:MM/DD'), [['first'], ['date', 'MM/DD']]);
    assert.deepEqual(pipesOf(''), []);
    assert.deepEqual(pipesOf(), []);
  });
});

describe('fill', () => {
  it('fills placeholders from values, applying their filters in order', () => {
    assert.equal(
      fill('Hi {{name|first|upper}}, born {{dob|date:YYYY}}', { name: 'Ada Lovelace', dob: '1815-12-10' }),
      'Hi ADA, born 1815',
    );
  });

  it('leaves unknown keys and unknown filters as typed', () => {
    assert.equal(fill('{{missing}} {{x|nope}}', { x: 'v' }), '{{missing}} v');
  });

  it('fills numbers and zero', () => {
    assert.equal(fill('{{n}}', { n: 0 }), '0');
  });

  it('passes non-strings through', () => {
    assert.equal(fill(42 as any, {}), 42);
    assert.equal(fill(undefined), undefined);
  });
});

describe('redact', () => {
  it('turns values back into their placeholders', () => {
    assert.equal(redact('ada@x.test signed in', { email: 'ada@x.test' }), '{{email}} signed in');
  });

  it('replaces the longest value first, so a value inside another is not split', () => {
    assert.equal(redact('Ada Lovelace', { first: 'Ada', full: 'Ada Lovelace' }), '{{full}}');
  });

  it('leaves values under three characters alone', () => {
    assert.equal(redact('NY is big', { state: 'NY' }), 'NY is big');
  });

  it('passes non-strings through and tolerates null values', () => {
    assert.equal(redact(null as any, { a: 'abc' }), null);
    assert.equal(redact('abc', { a: null }), 'abc');
  });
});

it('recognises a file value by its base64 body', () => {
  assert.equal(isFileValue({ file: 'a.pdf', type: 'application/pdf', b64: '' }), true);
  assert.equal(isFileValue({ file: 'a.pdf' }), false);
  assert.equal(isFileValue('a.pdf'), false);
  assert.equal(isFileValue(null), false);
});

it('takes a file name with or without placeholder braces', () => {
  assert.equal(dataKey('{{ resume }}'), 'resume');
  assert.equal(dataKey('resume'), 'resume');
  assert.equal(dataKey(undefined), '');
});
