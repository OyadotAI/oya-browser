/** Native obstructions require exact mounted identities, overlap and documented stacking order. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { inspectDebugMouseSurface } from '../../../../src/main/browser-debug/surface.ts';
import type { BrowserWindow, BrowserView } from 'electron';
import type { NativePage } from '../../../../src/main/native/index.ts';
const bounds = { x: 0, y: 50, width: 900, height: 600 };
const view = (x = 0) => ({ webContents: {}, getBounds: () => ({ ...bounds, x }) }) as unknown as BrowserView;
/** Minimal native identity and stacking surface, with no page or platform effects. */
function fixture(views: BrowserView[], contents = {}) {
  return {
    webContents: contents,
    getBrowserViews: () => views,
    getContentSize: () => [900, 650],
  } as unknown as BrowserWindow;
}
it('classifies only an overlapping shield above the exact owned page', () => {
  const owned = view(),
    shield = view(),
    page = owned as unknown as NativePage;
  assert.equal(inspectDebugMouseSurface(fixture([owned, shield]), page, shield).obstruction, 'browser_control_shield');
  assert.equal(inspectDebugMouseSurface(fixture([shield, owned]), page, shield).obstruction, undefined);
  assert.equal(inspectDebugMouseSurface(fixture([owned, view(1000)]), page, shield).obstruction, undefined);
});
it('classifies an overlapping personal view without attributing it to an unmounted shield', () => {
  const owned = view(),
    personal = view(),
    shield = view();
  assert.equal(
    inspectDebugMouseSurface(fixture([owned, personal]), owned as unknown as NativePage, shield).obstruction,
    'native_occlusion',
  );
});
it('never infers popup ownership from matching bounds or an absent native view', () => {
  const owned = view(),
    foreign = view(),
    page = owned as unknown as NativePage;
  assert.throws(() => inspectDebugMouseSurface(fixture([], foreign.webContents), page, null), /no longer mounted/);
  assert.equal(inspectDebugMouseSurface(fixture([], owned.webContents), page, null).obstruction, undefined);
  assert.equal(
    inspectDebugMouseSurface(fixture([foreign], owned.webContents), page, null).obstruction,
    'native_occlusion',
  );
});
