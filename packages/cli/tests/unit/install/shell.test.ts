/**
 * Unit tests for the machine probes (src/install/shell.ts): version floors,
 * captured output, and the port probe (on loopback only).
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:net';
import { atLeast, capture, has, portFree, run } from '../../../src/install/shell.ts';

describe('atLeast', () => {
  it('compares major then minor, digit-wise', () => {
    assert.equal(atLeast('Docker Compose version v2.24.1', 2, 24), true);
    assert.equal(atLeast('2.20.0', 2, 24), false);
    assert.equal(atLeast('5.0', 2, 24), true);
    assert.equal(atLeast(null, 2, 24), false);
  });
});

describe('capture and run', () => {
  it('capture returns trimmed stdout, or null on failure or a missing program', async () => {
    assert.equal(await capture(process.execPath, ['-e', 'console.log(" hi ")']), 'hi');
    assert.equal(await capture(process.execPath, ['-e', 'process.exit(3)']), null);
    assert.equal(await capture('oya-no-such-program', []), null);
    assert.equal(await has('oya-no-such-program'), false);
  });

  it('run rejects with the command line and exit code', async () => {
    await assert.rejects(
      run(process.execPath, ['-e', 'process.exit(2)'], { cwd: process.cwd(), quiet: true }),
      /exited 2$/,
    );
  });
});

describe('portFree', () => {
  it('is false while something listens, and true after', async () => {
    const server = createServer().listen(0, '127.0.0.1');
    await new Promise((r) => server.once('listening', r));
    const { port } = server.address() as { port: number };
    assert.equal(await portFree(port), false);
    await new Promise((r) => server.close(r));
    assert.equal(await portFree(port), true);
  });
});
