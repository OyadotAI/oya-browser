/**
 * Unit tests for quitting: a recording is saved before the app quits, a
 * validation running at quit is marked interrupted, and the cookie jar is on
 * disk before the app goes.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { Lifecycle } from '../../../../src/main/app/lifecycle.ts';
import { mainCtx } from '../../support/main-ctx.cjs';

describe('Lifecycle', () => {
  let ctx: any;
  beforeEach(() => {
    ctx = mainCtx();
    new Lifecycle(ctx).install();
  });

  /** Emits before-quit; returns whether it was held. */
  const quit = () => {
    const event = {
      held: false,
      preventDefault() {
        this.held = true;
      },
    };
    ctx.electron.app.emit('before-quit', event);
    return event.held;
  };

  it('holds the first quit until the recording is stopped, then quits', async () => {
    let stopped = false;
    ctx.recorder = {
      recording: true,
      queueRecording: (work) => Promise.resolve(work()),
      stopRecording: async () => (stopped = true),
    };
    assert.equal(quit(), true);
    await new Promise((r) => setImmediate(r));
    assert.equal(stopped, true);
    assert.equal(ctx.electron.app.quitted, true);
    assert.equal(quit(), true, 'the second quit waits for the jar');
    assert.equal(quit(), false, 'the third goes through');
  });

  it('holds the quit until the cookies and storage are on disk, then quits', async () => {
    const steps = [];
    ctx.persona.flushJar = async () => steps.push('jar');
    ctx.electron.app.quit = () => steps.push('quit');
    assert.equal(quit(), true);
    await new Promise((r) => setImmediate(r));
    assert.deepEqual(steps, ['jar', 'quit']);
    assert.equal(quit(), false, 'the quit after the flush goes through');
  });

  it('quits even when the jar cannot be written', async () => {
    ctx.persona.flushJar = async () => {
      throw new Error('disk full');
    };
    assert.equal(quit(), true);
    await new Promise((r) => setImmediate(r));
    assert.equal(ctx.electron.app.quitted, true);
  });

  it('marks a running validation interrupted and flushes the panel', () => {
    let flushed = false;
    const received = [];
    ctx.workspace = {
      busy: () => true,
      receive: (m) => received.push(m),
      session: {
        dispose() {
          this.disposed = true;
        },
      },
    };
    ctx.layout.flush = () => (flushed = true);
    quit();
    assert.equal(received[0].status, 'interrupted');
    assert.equal(ctx.workspace.session.disposed, true);
    assert.equal(flushed, true);
  });

  it('sends the cookie changes still queued, then disconnects and quits, when the last window closes', () => {
    let socketUpAtFlush = null;
    ctx.cookies.flushCookieChanges = () => (socketUpAtFlush = !ctx.socket.disconnects);
    ctx.electron.app.emit('window-all-closed');
    assert.equal(socketUpAtFlush, true, 'a login made a moment ago goes out while the socket is still up');
    assert.equal(ctx.socket.disconnects, 1);
    assert.equal(ctx.electron.app.quitted, true);
  });
});
