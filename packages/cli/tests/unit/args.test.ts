/**
 * Unit tests for argv parsing (src/args.ts).
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { parse, flagStr, flagNum } from '../../src/args.ts';

describe('parse', () => {
  it('defaults to help with no command', () => {
    assert.deepEqual(parse([]), { command: 'help', args: [], flags: {} });
  });

  it('splits positionals from flags, and a flag takes the next token as its value', () => {
    assert.deepEqual(parse(['goto', 'https://x', '--id', 'b1']), {
      command: 'goto',
      args: ['https://x'],
      flags: { id: 'b1' },
    });
  });

  it('makes a flag followed by another flag, or by nothing, true', () => {
    assert.deepEqual(parse(['rm', '--all', '--json']).flags, { all: true, json: true });
  });

  it('keeps positionals after flags', () => {
    assert.deepEqual(parse(['personas', '--json', 'x', 'new', 'n']).args, ['new', 'n']);
  });
});

describe('flag readers', () => {
  it('flagStr returns only string values', () => {
    assert.equal(flagStr({ a: 'x', b: true }, 'a'), 'x');
    assert.equal(flagStr({ a: 'x', b: true }, 'b'), undefined);
  });

  it('flagNum reads a number, or undefined when absent', () => {
    assert.equal(flagNum({ n: '30000' }, 'n'), 30000);
    assert.equal(flagNum({}, 'n'), undefined);
  });
});
