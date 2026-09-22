/**
 * Unit tests for how a failed command ends: exit 2 for a command that could not
 * be run and 1 for everything else (set, never forced, so piped output is
 * whole), one JSON line with --json, the CLI's own words for an unreachable
 * server and a rejected key naming where each came from, a shown partial
 * failure not printed twice, and --debug's extra lines.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { OyaError } from '@oya-ai/browser';
import { fail } from '../../src/failure.ts';
import { CliError, usage } from '../../src/errors.ts';
import { captured } from './support/harness.ts';

/** Runs fail() and returns what it printed and the exit code it set. */
async function failed(err: unknown, argv: string[] = []) {
  const { out, err: stderr } = await captured(() => fail(err, argv));
  const code = process.exitCode;
  process.exitCode = 0;
  return { out, stderr, code };
}

/** The status-0 error the SDK throws when nothing answered. */
const unreachable = (cause = 'ECONNREFUSED') => new OyaError('Could not reach x', 0, { code: 'unreachable', cause });

describe('fail', () => {
  const env = { key: process.env.OYA_API_KEY, url: process.env.OYA_BASE_URL };
  afterEach(() => {
    process.exitCode = 0;
    for (const [name, value] of [
      ['OYA_API_KEY', env.key],
      ['OYA_BASE_URL', env.url],
    ] as const) {
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
  });

  it('exits 2 for a usage or JSON error, 1 for anything else, by setting the exit code', async () => {
    assert.equal((await failed(usage('no'))).code, 2);
    assert.equal((await failed(new CliError('bad json', 'invalid_json'))).code, 2);
    assert.equal((await failed(new OyaError('nope', 422, { code: 'command_failed' }))).code, 1);
    assert.equal((await failed(new Error('boom'))).code, 1);
  });

  it('prints one line and an indented hint, with no blank line first', async () => {
    const { stderr, out } = await failed(new CliError('No API key.', 'no_api_key', { hint: 'Run oya login.' }));
    assert.equal(stderr, '✗ No API key.\n  Run oya login.');
    assert.equal(out, '');
  });

  it('prints one JSON object on stderr with --json, and nothing on stdout', async () => {
    const { stderr, out } = await failed(new OyaError('held', 409, { code: 'endpoint_in_use', browserId: 'b1' }), [
      '--json',
    ]);
    assert.deepEqual(JSON.parse(stderr), { error: 'held', code: 'endpoint_in_use', status: 409, browserId: 'b1' });
    assert.equal(out, '');
  });

  it('says why a server could not be reached and where the address came from', async () => {
    delete process.env.OYA_BASE_URL;
    const flag = await failed(unreachable(), ['ls', '--url', 'http://127.0.0.1:3202']);
    assert.equal(
      flag.stderr,
      [
        '✗ Could not reach http://127.0.0.1:3202: nothing is listening there.',
        '  The URL came from --url. Start the server there, or pass the right --url.',
      ].join('\n'),
    );
    process.env.OYA_BASE_URL = 'http://nowhere.test';
    const envUrl = await failed(unreachable('ENOTFOUND'), ['ls']);
    assert.match(
      envUrl.stderr,
      /^✗ Could not reach http:\/\/nowhere\.test: no such host\.\n {2}The URL came from OYA_BASE_URL\./,
    );
  });

  it('reads the source from --url=value too, and when parsing itself failed', async () => {
    const { stderr } = await failed(unreachable(), ['ls', '--url=http://127.0.0.1:9', '--bogus']);
    assert.match(stderr, /Could not reach http:\/\/127\.0\.0\.1:9: /);
  });

  it('names where a rejected key came from, for a 401 and for the 403 an unknown key gets', async () => {
    process.env.OYA_API_KEY = 'oya_env_key';
    for (const e of [new OyaError('x', 401, {}), new OyaError('Invalid API key', 403, { error: 'Invalid API key' })]) {
      const { stderr } = await failed(e, ['ls', '--url', 'http://h.test']);
      assert.match(stderr, /^✗ Invalid API key for http:\/\/h\.test \(the key came from OYA_API_KEY\)\./);
    }
  });

  it('gives a role refusal no login hint', async () => {
    const { stderr } = await failed(
      new OyaError('Viewers cannot stop sessions', 403, { error: 'Viewers cannot stop sessions' }),
    );
    assert.equal(stderr, '✗ Viewers cannot stop sessions');
  });

  it('does not print a partial failure a second time, but still exits 1', async () => {
    const { stderr, code } = await failed(new CliError('stopped 1 of 2', 'partial_failure', { shown: true }));
    assert.deepEqual([stderr, code], ['', 1]);
  });

  it('adds the status, the code, the body and the stack with --debug', async () => {
    const { stderr } = await failed(new OyaError('nope', 422, { code: 'command_failed', detail: 1 }), ['--debug']);
    assert.match(
      stderr,
      /^✗ nope\n {2}status 422 · code command_failed\n\{"code":"command_failed","detail":1\}\nOyaError: nope/,
    );
  });
});
