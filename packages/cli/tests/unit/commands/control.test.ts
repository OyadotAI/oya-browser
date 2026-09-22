/**
 * Unit tests for the control-plane commands (src/commands/control.ts): which
 * commands route there, what each calls, and the usage errors.
 */
import { FLAGS, captured, fakeFetch } from '../support/harness.ts';
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { isControlCommand, runControl } from '../../../src/commands/control.ts';

/** Runs a control command against the fake control plane; returns its calls and output. */
async function run(command: string, args: string[] = [], flags: Record<string, string | boolean> = {}) {
  const calls = fakeFetch(new Proxy({}, { get: () => ({ ok: true }) }) as Record<string, unknown>);
  const { out } = await captured(() => runControl(command, args, { ...FLAGS, ...flags }));
  return { calls: calls.map((c) => [c.method, c.path, c.body]), out };
}

describe('isControlCommand', () => {
  it('routes control commands, and stop only with --force', () => {
    assert.equal(isControlCommand('sessions', {}), true);
    assert.equal(isControlCommand('stop', {}), false);
    assert.equal(isControlCommand('stop', { force: true }), true);
    assert.equal(isControlCommand('ls', {}), false);
    assert.equal(isControlCommand('toString', {}), false);
  });
});

describe('runControl', () => {
  afterEach(() => mock.restoreAll());

  it('prints the answer as JSON', async () => {
    const { calls, out } = await run('control');
    assert.deepEqual(calls, [['GET', '/api/control', undefined]]);
    assert.equal(out, '{\n  "ok": true\n}');
  });

  it('maps takeover to acquire, and sends release and resume as named', async () => {
    assert.deepEqual((await run('takeover', ['s1'])).calls[0][2], { action: 'acquire' });
    assert.deepEqual((await run('release', ['s1'])).calls[0][2], { action: 'release' });
    assert.deepEqual((await run('resume', ['s1'])).calls[0][2], { action: 'resume' });
  });

  it('stop --force and recover --replace pass their flags', async () => {
    assert.deepEqual((await run('stop', ['s1'], { force: true })).calls[0], [
      'POST',
      '/api/control/sessions/s1/stop',
      { force: true },
    ]);
    assert.deepEqual((await run('recover', ['s1'], { replace: true })).calls[0][2], { replace: true });
  });

  it('reads events after a cursor, and project settings as JSON', async () => {
    assert.equal((await run('events', [], { after: '7' })).calls[0][1], '/api/control/events?after=7');
    assert.deepEqual((await run('project', ['{"auditDays":3}'])).calls[0], [
      'PATCH',
      '/api/control/project',
      { auditDays: 3 },
    ]);
  });

  it('invites members and mints credentials with a checked role', async () => {
    assert.deepEqual((await run('members', ['invite'])).calls[0][2], { role: 'operator' });
    assert.deepEqual((await run('credential', ['new'], { role: 'viewer', label: 'ci' })).calls[0][2], {
      role: 'viewer',
      label: 'ci',
    });
    await assert.rejects(run('members', ['invite'], { role: 'root' }), {
      code: 'usage',
      message: '--role must be viewer, operator or administrator, not "root".',
    });
  });

  it('runs webhook subcommands', async () => {
    assert.deepEqual((await run('webhook', ['new', 'https://h'])).calls[0], [
      'POST',
      '/api/control/webhooks',
      { url: 'https://h', types: [] },
    ]);
    assert.deepEqual((await run('webhook', ['replay', 'd1'])).calls[0][1], '/api/control/deliveries/d1/replay');
  });

  it('explains a missing argument or an unknown subcommand', async () => {
    await assert.rejects(run('cancel'), { code: 'usage', message: 'oya cancel needs a session id: oya cancel <id>.' });
    await assert.rejects(run('webhook', ['nope']), {
      message: 'Unknown webhook subcommand "nope". Use new, remove or replay.',
    });
    await assert.rejects(run('credential', ['nope']), {
      message: 'Unknown credential subcommand "nope". Use revoke or new.',
    });
    await assert.rejects(run('members', ['nope']), {
      message: 'Unknown members subcommand "nope". Use remove or invite.',
    });
    await assert.rejects(run('members', ['remove']), { code: 'usage', message: /^oya members remove needs a user id/ });
  });

  it('recovers with a replacement on a given Chrome, and refuses --ws-url without --replace', async () => {
    const recovered = await run('recover', ['s1'], { replace: true, 'ws-url': 'ws://127.0.0.1:9222' });
    assert.deepEqual(recovered.calls[0][2], { replace: true, wsUrl: 'ws://127.0.0.1:9222' });
    await assert.rejects(run('recover', ['s1'], { 'ws-url': 'ws://x' }), {
      code: 'usage',
      message: /only used with --replace/,
    });
  });

  it('names the bad JSON oya project was given, and how to quote it', async () => {
    await assert.rejects(run('project', ['{bad']), (e: { code: string; message: string }) => {
      assert.equal(e.code, 'invalid_json');
      assert.match(e.message, /^The settings for oya project are not valid JSON: /);
      return true;
    });
  });
});
