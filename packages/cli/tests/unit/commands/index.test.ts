/**
 * Unit tests for command dispatch (src/commands/index.ts): the command map,
 * aliases, help, and unknown commands.
 */
import { captured, trapExit, Exit } from '../support/harness.ts';
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { COMMANDS, dispatch } from '../../../src/commands/index.ts';

describe('dispatch', () => {
  afterEach(() => mock.restoreAll());

  it('prints the help for help, --help and -h', async () => {
    for (const command of ['help', '--help', '-h']) {
      const { out } = await captured(() => dispatch(command, [], {}));
      assert.match(out, /^oya, thousands of browsers, one API/);
    }
  });

  it('shares handlers between aliases', () => {
    assert.equal(COMMANDS.rm, COMMANDS.stop);
    assert.equal(COMMANDS['--help'], COMMANDS.help);
  });

  it('refuses an unknown command, including inherited names, with the help and exit 1', async () => {
    trapExit();
    for (const command of ['bogus', 'constructor']) {
      const { err, out } = await captured(() => assert.rejects(dispatch(command, [], {}), (e: Exit) => e.code === 1));
      assert.equal(err, `Unknown command: ${command}\n`);
      assert.match(out, /Options: --url/);
    }
  });

  it('whoami reports whether a key is saved, never the key', async () => {
    const { out } = await captured(() => dispatch('whoami', [], {}));
    assert.deepEqual(JSON.parse(out), { apiKey: 'none', baseUrl: 'https://oyabrowser.com' });
  });
});
