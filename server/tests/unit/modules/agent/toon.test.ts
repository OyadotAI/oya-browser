/**
 * Unit tests for the TOON writer: which cells are quoted, and the table shape.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { toonCell, toonTable } from '../../../../src/modules/agent/toon.ts';

describe('toonCell', () => {
  it('leaves plain text and numbers bare', () => {
    assert.equal(toonCell('From Date'), 'From Date');
    assert.equal(toonCell(12), '12');
    assert.equal(toonCell('10/23/2026'), '10/23/2026');
  });

  it('quotes what TOON would misread: empty, padded, keywords, numbers written as text', () => {
    assert.equal(toonCell(''), '""');
    assert.equal(toonCell(undefined), '""');
    assert.equal(toonCell(' x'), '" x"');
    assert.equal(toonCell('true'), '"true"');
    assert.equal(toonCell('42'), '"42"');
  });

  it('quotes and escapes text carrying delimiters or syntax', () => {
    assert.equal(toonCell('a, b'), '"a, b"');
    assert.equal(toonCell('invalid: "bad"'), '"invalid: \\"bad\\""');
    assert.equal(toonCell('-5 days'), '"-5 days"');
    assert.equal(toonCell('line\nbreak'), '"line\\nbreak"');
  });
});

describe('toonTable', () => {
  it('names the columns once and writes one row per item', () => {
    const rows = [
      { id: 1, label: 'Go' },
      { id: 2, label: 'a, b' },
    ];
    assert.equal(toonTable('items', ['id', 'label'], rows), 'items[2]{id,label}:\n  1,Go\n  2,"a, b"');
  });

  it('writes an empty table as its header alone', () => {
    assert.equal(toonTable('items', ['id'], []), 'items[0]{id}:');
  });
});
