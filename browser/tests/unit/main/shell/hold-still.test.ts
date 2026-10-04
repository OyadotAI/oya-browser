/**
 * Unit tests for holding the shell's pages still in a container: the media
 * emulation is sent only there, and a refused command does not throw.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { FakeView } from '../../support/fakes.cjs';
import { holdStill, inContainer } from '../../../../src/main/shell/hold-still.ts';

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
});
