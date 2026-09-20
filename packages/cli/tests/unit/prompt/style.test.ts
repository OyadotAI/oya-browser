/**
 * Unit tests for terminal styling (src/prompt/style.ts). Under the test
 * runner stdout is not a TTY, so every style is plain.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ESC, plain, style, up, visible } from '../../../src/prompt/style.ts';

describe('style', () => {
  it('leaves text alone off a terminal', () => {
    assert.equal(plain, true);
    assert.equal(style.green('ok'), 'ok');
  });

  it('measures printable width without colour codes', () => {
    assert.equal(visible(`${ESC}[32mhello${ESC}[0m`), 5);
  });

  it('moves the cursor up n rows', () => {
    assert.equal(up(3), `${ESC}[3A`);
  });
});
