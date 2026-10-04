/**
 * Unit tests for the Inspect Activity pane: an entry without a type is safe,
 * filters pick entries by direction and type, rows open unless a click only
 * ended a text selection, the log keeps its newest entries, and Clear empties.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  NetLogViewModel,
  shows,
  directionLabel,
  fmtTime,
} from '../../../../../../src/renderer/features/inspect/view-models/net-log-view-model.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';
import { fakeBridge } from '../../../support/bridge.ts';

/** An Activity pane over a fake bridge, with the Clear it registered. */
function log() {
  const fake = fakeBridge();
  const clears = new Map<string, () => void>();
  const panel = { onClear: (pane: string, fn: () => void) => (clears.set(pane, fn), () => {}), show() {} };
  return { fake, clears, vm: new NetLogViewModel({ bridge: fake.bridge, panel: panel as never }) };
}

describe('NetLogViewModel', () => {
  it('shows and filters a message that has no type', () => {
    const { fake, vm } = log();
    fake.emit('onDevLog', { ts: 0, dir: 'in', data: '{}' });
    assert.equal(vm.state.entries[0].type, 'message');
    assert.equal(shows('cmd', vm.state.entries[0]), false);
    assert.equal(shows('all', vm.state.entries[0]), true);
  });

  it('filters by direction, commands and results; an unknown filter shows everything', () => {
    const cmd = { dir: 'out', type: 'cmd: click' };
    assert.deepEqual(
      ['in', 'out', 'cmd', 'result', 'nope'].map((f) => shows(f, cmd)),
      [false, true, true, false, true],
    );
    assert.equal(shows('cmd', { dir: 'in', type: 'auth' }), true);
    assert.equal(shows('result', { dir: 'in', type: 'result: ok' }), true);
  });

  it('switches the filter', () => {
    const { vm } = log();
    vm.setFilter('cmd');
    assert.equal(vm.state.filter, 'cmd');
  });

  it('opens and closes a row on a click', () => {
    const { fake, vm } = log();
    for (let i = 0; i < 3; i++) fake.emit('onDevLog', { ts: 0, dir: 'out', type: 'cmd: click', data: '' });
    const key = vm.state.entries[1].key;
    vm.toggleRow(key);
    assert.deepEqual(vm.state.expanded, [key]);
    vm.toggleRow(key);
    assert.deepEqual(vm.state.expanded, []);
  });

  it('keeps a row open when the click only ended a text selection', () => {
    const { fake, vm } = log();
    fake.emit('onDevLog', { ts: 0, dir: 'in', type: 'auth', data: 'long body' });
    const key = vm.state.entries[0].key;
    vm.toggleRow(key);
    vm.toggleRow(key, 'long');
    assert.deepEqual(vm.state.expanded, [key]);
  });

  it('is empty until a message comes, and again after Clear', () => {
    const { fake, clears, vm } = log();
    assert.equal(vm.state.entries.length, 0);
    fake.emit('onDevLog', { ts: 0, dir: 'in', type: 'auth', data: '' });
    assert.equal(vm.state.entries.length, 1);
    clears.get('network')?.();
    assert.equal(vm.state.entries.length, 0);
  });

  it('keeps only the newest entries', () => {
    const { vm } = log();
    for (let i = 0; i <= C.NET_LOG_LIMIT; i++) vm.add({ ts: i, dir: 'in', type: 'x', data: String(i) });
    assert.equal(vm.state.entries.length, C.NET_LOG_LIMIT);
    assert.equal(vm.state.entries[0].data, '1');
  });

  it('names the direction in words', () => {
    assert.equal(directionLabel('out'), 'To server');
    assert.equal(directionLabel('in'), 'From server');
  });

  it('tells the time to the millisecond', () => {
    assert.match(fmtTime(Date.now()), /^\d\d:\d\d:\d\d\.\d{3}$/);
  });

  it('stops hearing messages once disposed', () => {
    const { fake, vm } = log();
    vm.dispose();
    assert.equal(fake.listeners('onDevLog'), 0);
  });
});
