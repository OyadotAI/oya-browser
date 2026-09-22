/**
 * Unit tests for the help text against what the CLI really has: every command
 * in the command maps is in it, and its flags are exactly the flags the parser
 * knows, so neither side can grow without the other.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { HELP } from '../../src/help.ts';
import { COMMANDS } from '../../src/commands/index.ts';
import { CONTROL_COMMANDS } from '../../src/commands/control.ts';
import { SWITCHES, VALUE_FLAGS } from '../../src/args.ts';

/** Names that print the help or the version rather than being listed as commands. */
const NOT_LISTED = new Set(['help', '--help', '-h', 'version']);

describe('oya help', () => {
  it('lists every command in both command maps', () => {
    for (const name of [...Object.keys(COMMANDS), ...Object.keys(CONTROL_COMMANDS)]) {
      if (NOT_LISTED.has(name)) continue;
      assert.ok(new RegExp(`oya ${name}\\b|\\| ${name}\\b`).test(HELP), `oya help does not list ${name}`);
    }
  });

  it('names exactly the flags the parser knows', () => {
    const inHelp = new Set([...HELP.matchAll(/--([a-z][a-z-]*)/g)].map((m) => m[1]));
    const known = new Set([...SWITCHES, ...Object.keys(VALUE_FLAGS)]);
    assert.deepEqual([...inHelp].filter((f) => !known.has(f)).sort(), [], 'in the help but unknown to the parser');
    assert.deepEqual([...known].filter((f) => !inHelp.has(f)).sort(), [], 'known to the parser but not in the help');
  });

  it('says what the exit codes mean', () => {
    assert.match(HELP, /Exit codes: 0 done · 1 failed, or no answer · 2 usage error, nothing was sent/);
  });
});
