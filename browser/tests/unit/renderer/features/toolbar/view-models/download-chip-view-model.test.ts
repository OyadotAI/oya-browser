/**
 * Unit tests for the toolbar's download chip: it says "Saved" only for a
 * completed download and "Failed" only for one that ended, announces each
 * change once, holds an ended download while the person may not be looking,
 * puts it away after its wait, and acts on the file by id.
 */
import { it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { DownloadChipViewModel } from '../../../../../../src/renderer/features/toolbar/view-models/download-chip-view-model.ts';
import {
  CHIP_ARM_MS,
  CHIP_DONE_MS,
  CHIP_FAILED_MS,
} from '../../../../../../src/renderer/features/toolbar/model/constants.ts';
import { fakeBridge } from '../../../support/bridge.ts';

beforeEach(() => mock.timers.enable({ apis: ['setTimeout'] }));
afterEach(() => mock.timers.reset());

/** A download entry as main lists it. */
const entry = (state: string, over: Record<string, unknown> = {}) => ({
  ...{ id: 'd1', url: 'https://files.test/report.pdf', title: 'report.pdf', time: 1 },
  ...{ state, received: 1024, total: 2048, ...over },
});

/** What main sends a window: its latest download and the running totals. */
const bubble = (latest: unknown, over: Record<string, unknown> = {}) => {
  const running = (latest as any)?.state === 'progressing' ? 1 : 0;
  return { latest, running, received: running ? 1024 : 0, total: running ? 2048 : 0, focused: true, ...over };
};

/** A chip on a Mac over a fake bridge; `answers` answers its calls. */
function chip(answers: Record<string, unknown> = {}) {
  const fake = fakeBridge(answers);
  const vm = new DownloadChipViewModel(fake.bridge as any, 'MacIntel');
  return {
    fake,
    vm,
    send: (latest: unknown, over?: Record<string, unknown>) => fake.emit('onDownloadBubble', bubble(latest, over)),
  };
}

it('is the plain Downloads button until a download starts here', () => {
  const { vm } = chip();
  assert.deepEqual([vm.state.phase, vm.state.label, vm.state.ring], ['idle', 'Downloads', false]);
});

it('a running download shows its name, its bytes and a ring', () => {
  const { vm, send } = chip();
  send(entry('progressing'));
  const s = vm.state;
  assert.deepEqual([s.phase, s.status, s.stem, s.ext, s.ring], ['running', 'Downloading', 'report', '.pdf', 0.5]);
  assert.equal(s.label, 'Downloading report.pdf, 1.0 KB of 2.0 KB. Show all downloads');
  assert.equal(s.meter, '1.0 KB of 2.0 KB · 50%');
  assert.equal(s.announce, '', 'a download first seen running waits for news that it still is');
  send(entry('progressing', { received: 1536 }), { received: 1536 });
  assert.equal(vm.state.announce, 'Downloading report.pdf');
  assert.equal(vm.state.meter, '1.5 KB of 2.0 KB · 75%');
});

it('a download that ends at once is announced only as Saved, never Downloading first', () => {
  const { vm, send } = chip();
  send(entry('progressing'));
  send(entry('completed', { received: 2048 }));
  assert.equal(vm.state.announce, 'Saved report.pdf');
  assert.equal(vm.state.meter, '', 'no meter once nothing runs');
});

it('a size the site did not give says how much so far, with a sliding bar', () => {
  const { vm, send } = chip();
  send(entry('progressing', { total: 0 }), { total: null });
  assert.deepEqual([vm.state.ring, vm.state.meter], [null, '1.0 KB so far']);
});

it('a size the site did not give turns the ring and says so far', () => {
  const { vm, send } = chip();
  send(entry('progressing', { total: 0 }), { total: null });
  assert.equal(vm.state.ring, null);
  assert.match(vm.state.label, /1.0 KB so far/);
});

it('a download already ended on first sight goes straight to Saved and announces only that', () => {
  const { vm, send } = chip();
  send(entry('completed', { received: 2048 }));
  assert.deepEqual([vm.state.phase, vm.state.status, vm.state.ring], ['saved', 'Saved', false]);
  assert.equal(vm.state.label, 'Saved report.pdf, 2.0 KB. Show all downloads');
  assert.equal(vm.state.announce, 'Saved report.pdf');
});

it('says Failed, Cancelled or Saved only from how main says the download ended', () => {
  const { vm, send } = chip();
  for (const [state, phase, said] of [
    ['interrupted', 'failed', "report.pdf didn't download"],
    ['cancelled', 'cancelled', 'report.pdf was cancelled'],
  ]) {
    send(entry(state, { id: state }));
    assert.deepEqual([vm.state.phase, vm.state.announce], [phase, said]);
  }
});

it('several running show their count, not one name', () => {
  const { vm, send } = chip();
  send(entry('progressing'), { running: 3 });
  assert.deepEqual([vm.state.status, vm.state.named], ['Downloading 3 files', false]);
});

it('Saved and Cancelled go back to the plain button after their wait', () => {
  const { vm, send } = chip();
  send(entry('completed'));
  mock.timers.tick(CHIP_DONE_MS - 1);
  assert.equal(vm.state.phase, 'saved');
  mock.timers.tick(1);
  assert.equal(vm.state.phase, 'idle');
});

it('Failed waits longer, then goes too', () => {
  const { vm, send } = chip();
  send(entry('interrupted'));
  mock.timers.tick(CHIP_DONE_MS);
  assert.equal(vm.state.phase, 'failed');
  mock.timers.tick(CHIP_FAILED_MS - CHIP_DONE_MS);
  assert.equal(vm.state.phase, 'idle');
});

it('the pointer or focus on the chip holds it, and letting go restarts the full wait', () => {
  const { vm, send } = chip();
  send(entry('completed'));
  mock.timers.tick(CHIP_DONE_MS - 1);
  vm.pointer(true);
  mock.timers.tick(CHIP_DONE_MS * 2);
  vm.pointer(false);
  vm.focus(true);
  vm.focus(false);
  mock.timers.tick(CHIP_DONE_MS - 1);
  assert.equal(vm.state.phase, 'saved');
  mock.timers.tick(1);
  assert.equal(vm.state.phase, 'idle');
});

it('a window in the background holds the chip until the person comes back', () => {
  const { vm, send } = chip();
  send(entry('completed'), { focused: false });
  mock.timers.tick(CHIP_FAILED_MS);
  assert.equal(vm.state.phase, 'saved');
  send(entry('completed'), { focused: true });
  mock.timers.tick(CHIP_DONE_MS);
  assert.equal(vm.state.phase, 'idle');
});

it('news about other downloads does not restart the wait', () => {
  const { vm, send } = chip();
  send(entry('completed'));
  mock.timers.tick(CHIP_DONE_MS - 1);
  send(entry('completed'), { running: 1, received: 5, total: 10 });
  mock.timers.tick(1);
  assert.equal(vm.state.phase, 'idle');
  assert.equal(vm.state.ring, 0.5, 'the ring stays while another download runs');
});

it('a put-away download stays away; the next one takes the chip over', () => {
  const { vm, send } = chip();
  send(entry('completed'));
  assert.equal(vm.escape(), true);
  send(entry('completed'));
  assert.equal(vm.state.phase, 'idle');
  send(entry('progressing', { id: 'd2' }));
  assert.equal(vm.state.phase, 'running');
});

it('Escape does nothing while a download runs', () => {
  const { vm, send } = chip();
  send(entry('progressing'));
  assert.equal(vm.escape(), false);
  assert.equal(vm.state.phase, 'running');
});

it('the body opens the Downloads page and puts an ended download away', () => {
  const { vm, send, fake } = chip();
  send(entry('completed'));
  vm.showAll();
  assert.deepEqual(fake.called('showLibrary'), [['downloads']]);
  assert.equal(vm.state.phase, 'idle');
});

it('Open and Show act on the file by id, then put the chip away', async () => {
  const { vm, send, fake } = chip();
  send(entry('completed'));
  mock.timers.tick(CHIP_ARM_MS);
  await vm.act('reveal', 'd1');
  assert.deepEqual(fake.called('downloadAction'), [['d1', 'reveal']]);
  assert.equal(vm.state.phase, 'idle');
});

it('a file moved or deleted since says so and stays until its longer wait', async () => {
  const { vm, send } = chip({ downloadAction: () => Promise.reject(Error('This file was moved or deleted')) });
  send(entry('completed'));
  mock.timers.tick(CHIP_ARM_MS);
  await vm.act('open', 'd1');
  assert.deepEqual([vm.state.phase, vm.state.status], ['gone', 'Moved or deleted']);
  assert.equal(vm.state.announce, 'That file was moved or deleted.');
  mock.timers.tick(CHIP_DONE_MS);
  assert.equal(vm.state.phase, 'gone');
});

it('nothing to show (another profile on screen) is the plain button at once', () => {
  const { vm, send } = chip();
  send(entry('completed'));
  send(null);
  assert.equal(vm.state.phase, 'idle');
});

it('Show names Finder on a Mac and folder elsewhere', () => {
  const fake = fakeBridge();
  assert.equal(new DownloadChipViewModel(fake.bridge as any, 'MacIntel').state.mac, true);
  assert.equal(new DownloadChipViewModel(fake.bridge as any, 'Win32').state.mac, false);
});

it('stops listening and waiting when disposed', () => {
  const { vm, send, fake } = chip();
  send(entry('completed'));
  vm.dispose();
  assert.equal(fake.listeners('onDownloadBubble'), 0);
  assert.doesNotThrow(() => mock.timers.tick(CHIP_FAILED_MS));
});

it('Open and Show ignore a click on a file that just arrived: a page cannot swap one in under a click', async () => {
  const { vm, send, fake } = chip();
  send(entry('completed'));
  mock.timers.tick(CHIP_ARM_MS - 1);
  await vm.act('open', 'd1');
  assert.equal(fake.called('downloadAction').length, 0);
  mock.timers.tick(1);
  await vm.act('open', 'd1');
  assert.deepEqual(fake.called('downloadAction'), [['d1', 'open']]);
});

it('Open and Show act only on the file that was on screen when clicked', async () => {
  const { vm, send, fake } = chip();
  send(entry('completed'));
  mock.timers.tick(CHIP_ARM_MS);
  send(entry('completed', { id: 'page-file' }));
  await vm.act('open', 'd1');
  mock.timers.tick(CHIP_ARM_MS);
  await vm.act('open', 'd1');
  assert.equal(fake.called('downloadAction').length, 0, 'the click was meant for d1, which is no longer shown');
});
