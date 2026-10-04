/**
 * Unit tests for the workspace panel: panes and the Inspect tab's memory,
 * opening and toggling, Clear per pane, and resizing by drag (a width a frame)
 * and by keyboard.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { PanelViewModel, widthAt, keyWidth } from '../../../../../src/renderer/app/panel/panel-view-model.ts';
import { RendererConstants as C } from '../../../../../src/renderer/core/constants.ts';
import { fakeBridge, manualFrames } from '../../support/bridge.ts';

/** A panel over a fake bridge and a hand-driven frame clock. */
function panel() {
  const fake = fakeBridge();
  const frames = manualFrames();
  return { fake, frames, vm: new PanelViewModel(fake.bridge, frames) };
}

describe('PanelViewModel', () => {
  it('follows the main process: open state and layout', () => {
    const { fake, vm } = panel();
    fake.emit('onDevPanelState', true);
    const layout = {
      compact: false,
      panelWidth: 400,
      panelHeight: 0,
      progress: 1,
      reveal: 400,
      chromeHeight: 80,
      page: { x: 0, y: 80, width: 600, height: 600 },
    };
    fake.emit('onShellLayout', layout);
    assert.equal(vm.state.open, true);
    assert.deepEqual(vm.state.layout, layout);
  });

  it('remembers the Inspect pane last shown, for the Inspect tab', () => {
    const { vm } = panel();
    vm.show('network');
    vm.show('chat');
    vm.showInspect();
    assert.equal(vm.state.pane, 'network');
  });

  it('opens on the asked pane, toggling only a closed panel', async () => {
    const { fake, vm } = panel();
    await vm.open('routines');
    assert.equal(vm.state.pane, 'routines');
    fake.emit('onDevPanelState', true);
    await vm.open('chat');
    assert.equal(fake.called('toggleDevPanel').length, 1);
  });

  it('opens on Record while recording', async () => {
    const { vm } = panel();
    await vm.toggle(true);
    assert.equal(vm.state.pane, 'record');
  });

  it('clears the pane in view only, and leaves a pane without a clear alone', () => {
    const { vm } = panel();
    const cleared: string[] = [];
    vm.onClear('chat', () => cleared.push('chat'));
    vm.onClear('network', () => cleared.push('network'));
    vm.clear();
    vm.show('routines');
    vm.clear();
    assert.deepEqual(cleared, ['chat']);
  });

  it('sends a dragged width at most once a frame, and the last one when the drag ends', () => {
    const { fake, frames, vm } = panel();
    vm.dragTo(100, 1400);
    assert.equal(frames.waiting, 0, 'no drag yet');
    vm.startDrag();
    vm.dragTo(1000, 1400);
    vm.dragTo(990, 1400);
    frames.run();
    vm.dragTo(980, 1400);
    vm.endDrag();
    assert.deepEqual(fake.called('resizeDevPanel'), [[410], [420]]);
    assert.equal(vm.state.dragging, false);
  });

  it('keeps a dragged width within the panel limits and the page minimum', () => {
    assert.equal(widthAt(1390, 1400), C.PANEL_MIN_WIDTH);
    assert.equal(widthAt(0, 1400), C.PANEL_MAX_WIDTH);
    assert.equal(widthAt(0, 900), 900 - C.PAGE_MIN_WIDTH);
  });

  it('resizes by key: Home and End to the limits, arrows by a step', () => {
    assert.equal(keyWidth('Home', 400), C.PANEL_MIN_WIDTH);
    assert.equal(keyWidth('End', 400), C.PANEL_MAX_WIDTH);
    assert.equal(keyWidth('ArrowLeft', 400), 400 + C.PANEL_KEY_STEP);
    assert.equal(keyWidth('ArrowRight', 400), 400 - C.PANEL_KEY_STEP);
  });

  it('stops following the main process once disposed', () => {
    const { fake, vm } = panel();
    vm.dispose();
    assert.equal(fake.listeners('onDevPanelState'), 0);
    assert.equal(fake.listeners('onShellLayout'), 0);
  });
});
