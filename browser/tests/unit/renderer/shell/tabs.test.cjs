/**
 * Unit tests for the tab strip: it keeps the main process's order, shows the
 * right icon, folds closed tabs away, holds widths after a close with the
 * mouse, and answers Delete, middle-click and right-click.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer } = require('../../support/renderer-harness.cjs');

/** A tab as the main process summarizes it. */
const tab = (id, extra = {}) => ({ id, title: `Tab ${id}`, url: `https://${id}.test/`, active: false, ...extra });

describe('the tab strip', () => {
  let app;
  /** The ids on the strip, in order, without tabs folding away. */
  const ids = () => [
    ...app.run("[...document.querySelectorAll('#tab-list .tab-item:not(.closing)')].map((n) => n.dataset.id)"),
  ];
  /** The item for tab `id`. */
  const item = (id) => app.document.querySelector(`#tab-list .tab-item[data-id="${id}"]`);
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    app = loadRenderer();
    app.bridge.emit('TabsUpdated', [tab(1, { active: true }), tab(2), tab(3)]);
  });
  afterEach(() => mock.timers.reset());

  it("follows the main process's order, which a drag or a shortcut changed", () => {
    app.bridge.emit('TabsUpdated', [tab(3), tab(1, { active: true }), tab(2)]);
    assert.deepEqual(ids(), ['3', '1', '2']);
  });

  it('keeps one tab focusable, the active one, with its selection on the tab button', () => {
    const buttons = app.document.querySelectorAll('#tab-list [role="tab"]');
    assert.deepEqual(
      buttons.map((b) => [b.getAttribute('aria-selected'), b.tabIndex]),
      [
        ['true', 0],
        ['false', -1],
        ['false', -1],
      ],
    );
  });

  it("shows a spinner while loading, the page's favicon, the Oya mark at home, else a globe", () => {
    const icon = (id) => item(id).querySelector('.tab-icon');
    app.bridge.emit('TabsUpdated', [
      tab(1, { active: true, loading: true }),
      tab(2, { favicon: 'data:image/png;base64,AA' }),
      tab(3, { home: true }),
      tab(4),
    ]);
    assert.ok(icon(1).querySelector('.tab-spinner'));
    assert.equal(icon(2).querySelector('img').src, 'data:image/png;base64,AA');
    assert.equal(icon(2).querySelector('img').draggable, false, 'an image would start a drag-and-drop');
    assert.ok(icon(3).querySelector('.tab-mark'));
    assert.ok(icon(4).querySelector('svg.icon'));
  });

  it('folds a closed tab away: no longer a tab at once, gone after the fold', () => {
    app.bridge.emit('TabsUpdated', [tab(1, { active: true }), tab(3)]);
    assert.ok(item(2).classList.contains('closing'));
    assert.equal(item(2).querySelector('.tab-title').getAttribute('role'), null);
    mock.timers.tick(app.run('RendererConstants.TAB_CLOSE_MS'));
    assert.equal(item(2), null);
  });

  it('removes a closed tab at once with reduced motion', () => {
    app = loadRenderer({ reducedMotion: true });
    app.bridge.emit('TabsUpdated', [tab(1, { active: true }), tab(2)]);
    app.bridge.emit('TabsUpdated', [tab(1, { active: true })]);
    assert.equal(item(2), null);
  });

  it('holds tab widths after a close with the mouse until the pointer leaves the strip', () => {
    app.run('TabStrip.room = () => 600');
    app.bridge.emit('TabsUpdated', [tab(1, { active: true }), tab(2), tab(3)]);
    const width = () => app.$('tab-list').style['--tab-width'];
    assert.equal(width(), '200px');
    app.fire(item(2).querySelector('.tab-close'), 'click');
    assert.deepEqual(app.bridge.called('closeTab'), [[2]]);
    app.bridge.emit('TabsUpdated', [tab(1, { active: true }), tab(3)]);
    assert.equal(width(), '200px', 'the next close button lands under the pointer');
    app.fire(app.$('tab-bar'), 'pointerleave');
    assert.equal(width(), '240px');
  });

  it('closes on middle-click and on Delete, and asks for the native menu on right-click', () => {
    app.fire(item(3), 'auxclick', { button: 1 });
    app.key(item(1).querySelector('.tab-title'), 'Delete');
    app.fire(item(2), 'contextmenu');
    assert.deepEqual(app.bridge.called('closeTab'), [[3], [1]]);
    assert.deepEqual(app.bridge.called('showTabMenu'), [[2]]);
  });
});
