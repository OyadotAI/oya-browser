/**
 * Unit tests for holding the shell's pages still in a container: the media
 * emulation is sent only there, and a refused command does not throw.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { FakeView } from '../../support/fakes.cjs';
import { holdStill, inContainer, stillWhileAway } from '../../../../src/main/shell/hold-still.ts';

/** A CDP command the fake debugger recorded. */
interface Sent {
  /** What the command was sent with. */
  params: unknown;
}

/** A sent CDP command's parameters. */
function paramsOf(sent: Sent) {
  return sent.params;
}

describe('holdStill', () => {
  it('tells the page to prefer reduced motion in a container', async () => {
    const view = new FakeView();
    await holdStill(view, true);
    const [sent] = view.webContents.debugger.sent;
    assert.equal(sent.method, 'Emulation.setEmulatedMedia');
    assert.deepEqual(sent.params, { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });
  });

  it('leaves the page alone outside a container', async () => {
    const view = new FakeView();
    await holdStill(view, false);
    assert.equal(view.webContents.debugger.sent.length, 0);
  });

  it('does not throw when the page is gone', async () => {
    const view = new FakeView();
    view.webContents.destroy();
    await holdStill(view, true);
  });

  it('knows a container by OYA_DOCKER', () => {
    assert.equal(inContainer({ OYA_DOCKER: 'true' }), true);
    assert.equal(inContainer({}), false);
  });

  it('holds still while another app is in front, and moves again on return', async () => {
    const events = new EventEmitter();
    const win = Object.assign(new FakeView(), {
      /** The window's focus events, as Electron's. */
      on: (name: string, fn: () => void) => events.on(name, fn),
    });
    stillWhileAway(win);
    events.emit('blur');
    events.emit('focus');
    await new Promise((resolve) => setImmediate(resolve));
    assert.deepEqual(win.webContents.debugger.sent.map(paramsOf), [
      { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] },
      { features: [] },
    ]);
  });
});
