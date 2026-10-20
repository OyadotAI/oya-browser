/**
 * Unit tests for src/main/shell/shield-press.ts: a person's press on the control
 * shield is logged once per control state and shown tab, and only mouse presses count.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { logShieldPresses } from '../../../../src/main/shell/shield-press.ts';

/** A shield page that hands its 'input-event' listener to the test. */
function shieldContents() {
  const contents: any = { listener: null };
  contents.on = (name, listener) => name === 'input-event' && (contents.listener = listener);
  contents.press = (type = 'mouseDown') => contents.listener({}, { type });
  return contents;
}

describe('logShieldPresses', () => {
  let presses, state, deps, contents;
  beforeEach(() => {
    presses = [];
    state = { mode: 'paused', mine: false, revision: 3, busy: false, interactive: false };
    deps = {
      control: { snapshot: () => state },
      tabs: { activeTabId: 1, list: [] },
      shell: { window: { getBrowserViews: () => ['page', 'shield'] } },
      tabHealthLog: { shieldPress: (press) => presses.push(press) },
    };
    contents = shieldContents();
    logShieldPresses(deps, contents, 'shield' as any);
  });

  it('logs the first mouse press on the shield', () => {
    contents.press();
    assert.equal(presses.length, 1);
    assert.deepEqual([presses[0].tab, presses[0].views, presses[0].shield], [1, ['page', 'shield'], 'shield']);
    assert.equal(presses[0].control, state);
  });

  it('logs nothing more under the same control state and tab', () => {
    for (let i = 0; i < 20; i++) contents.press();
    assert.equal(presses.length, 1);
  });

  it('logs again once the control state or the tab changes', () => {
    contents.press();
    state = { ...state, revision: 4 };
    contents.press();
    deps.tabs.activeTabId = 2;
    contents.press();
    assert.equal(presses.length, 3);
  });

  it('ignores keys and mouse moves', () => {
    for (const type of ['keyDown', 'mouseMove', 'mouseUp', 'mouseWheel']) contents.press(type);
    assert.equal(presses.length, 0);
  });

  it('does nothing without a health log', () => {
    delete deps.tabHealthLog;
    assert.doesNotThrow(() => contents.press());
  });
});
