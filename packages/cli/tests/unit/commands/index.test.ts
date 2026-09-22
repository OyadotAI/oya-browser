/**
 * Unit tests for command dispatch (src/commands/index.ts): the command map,
 * aliases, help, and unknown commands.
 */
import { captured } from '../support/harness.ts';
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

  it('asking for help anywhere on the line shows the help and runs nothing, whatever the command', async () => {
    const ran = mock.fn(async () => {});
    for (const name of ['personas', 'rm', 'install', 'cookies']) mock.method(COMMANDS, name, ran);
    const lines: [string, string[], Record<string, string | boolean>][] = [
      ['personas', ['new', 'n'], { help: true }],
      ['rm', [], { all: true, help: true }],
      ['install', [], { help: 'now' }],
      ['cookies', ['clear', 'p', '-h'], {}],
      ['credential', ['new', 'x'], { help: true }],
    ];
    for (const [command, args, flags] of lines) {
      const { out } = await captured(() => dispatch(command, args, flags));
      assert.match(out, /^oya, thousands of browsers, one API/, command);
    }
    assert.equal(ran.mock.callCount(), 0);
  });

  it('shares handlers between aliases', () => {
    assert.equal(COMMANDS.rm, COMMANDS.stop);
    assert.equal(COMMANDS['--help'], COMMANDS.help);
  });

  it('refuses an unknown command, including inherited names, as a usage error without printing the help', async () => {
    for (const command of ['bogus', 'constructor']) {
      const { out } = await captured(() =>
        assert.rejects(dispatch(command, [], {}), {
          code: 'usage',
          message: `Unknown command "${command}". Run oya help for the list.`,
        }),
      );
      assert.equal(out, '');
    }
  });

  it('prints the version, as JSON with --json', async () => {
    const { version } = JSON.parse(
      (await import('node:fs')).readFileSync(new URL('../../../package.json', import.meta.url), 'utf8'),
    );
    assert.equal((await captured(() => dispatch('version', [], {}))).out, `oya ${version}`);
    assert.equal((await captured(() => dispatch('--version', [], {}))).out, `oya ${version}`);
    assert.deepEqual(JSON.parse((await captured(() => dispatch('ls', [], { version: true, json: true }))).out), {
      version,
    });
  });

  it('refuses --json for the commands a person reads, before they start', async () => {
    for (const command of ['install', 'init', 'stealth-test'])
      await assert.rejects(dispatch(command, [], { json: true }), {
        code: 'usage',
        message: new RegExp(`^oya ${command} has no --json output`),
      });
  });

  it('whoami says where the key and address came from, never the key', async () => {
    const saved = process.env.OYA_API_KEY;
    process.env.OYA_API_KEY = 'oya_live_0123456789abcdef';
    try {
      const { out } = await captured(() => dispatch('whoami', [], {}));
      const who = JSON.parse(out);
      assert.deepEqual([who.apiKeyFrom, who.apiKeyEnds, who.baseUrlFrom], ['OYA_API_KEY', 'cdef', 'default']);
      assert.doesNotMatch(out, /0123456789/);
    } finally {
      if (saved === undefined) delete process.env.OYA_API_KEY;
      else process.env.OYA_API_KEY = saved;
    }
  });
});
