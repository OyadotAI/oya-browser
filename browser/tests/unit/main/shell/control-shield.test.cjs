/**
 * Unit tests for ControlShield: the transparent shield over the page while an
 * agent drives, popups and menu items fenced, and the human-control guard.
 */
const { describe, it, beforeEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { ControlShield } = require('../../../../main/shell/control-shield.cjs');
const { mainCtx, FakeBrowserView } = require('../../support/main-ctx.cjs');

describe('ControlShield', () => {
  let ctx, page;
  beforeEach(() => {
    ctx = mainCtx({ shield: ControlShield });
    page = new FakeBrowserView();
    page.setBounds({ x: 0, y: 88, width: 900, height: 700 });
    ctx.tabs = { getActiveView: () => page };
    ctx.shell.window.setBrowserView(page);
  });

  it('covers the page exactly while an agent has control', () => {
    ctx.control.state.interactive = false;
    ctx.shield.sync();
    assert.equal(ctx.shell.window.views.at(-1), ctx.shield.view);
    assert.deepEqual(ctx.shield.view.bounds, page.bounds);
    assert.equal(ctx.shield.view.background, '#00000000');
  });

  it('comes off when a person takes control, and under an overlay', () => {
    ctx.control.state.interactive = false;
    ctx.shield.sync();
    ctx.overlays.names.add('shell');
    ctx.shield.sync();
    assert.ok(!ctx.shell.window.views.includes(ctx.shield.view));
    ctx.overlays.names.clear();
    ctx.control.state.interactive = true;
    ctx.shield.sync();
    assert.ok(!ctx.shell.window.views.includes(ctx.shield.view));
  });

  it('refuses page actions unless a person has control', () => {
    ctx.shield.requireHumanControl();
    ctx.control.state.interactive = false;
    assert.throws(() => ctx.shield.requireHumanControl(), /Take control before interacting with this page/);
  });

  it('fences popups and page menu items when control changes', () => {
    const popup = Object.assign(new EventEmitter(), {
      enabled: null,
      setEnabled(v) {
        this.enabled = v;
      },
      isDestroyed: () => false,
    });
    ctx.shield.adoptPopup(popup);
    const item = { enabled: true };
    ctx.electron.Menu.items.set('browser-reload', item);
    const lost = mock.method(ctx.recorder, 'controlLost', () => {});
    ctx.shield.controlChanged({ interactive: false });
    assert.equal(popup.enabled, false);
    assert.equal(item.enabled, false);
    assert.equal(lost.mock.callCount(), 1);
    assert.deepEqual(ctx.shell.sentOn('control-state'), [{ interactive: false }]);
  });

  it('forgets a popup once it closes', () => {
    const popup = Object.assign(new EventEmitter(), { setEnabled() {} });
    ctx.shield.adoptPopup(popup);
    popup.emit('closed');
    assert.equal(ctx.shield.popups.size, 0);
  });

  it('hands focus back to the shell while an agent drives', () => {
    ctx.control.state.interactive = false;
    ctx.shield.keepFocusOnShell();
    assert.deepEqual(ctx.shell.window.webContents.calls, ['focus']);
  });

  describe('showing an analysis', () => {
    let told;
    /** Puts the shield up and records what its page is told. */
    const cover = () => {
      ctx.control.state.interactive = false;
      ctx.shield.sync();
      told = [];
      mock.method(ctx.shield.view.webContents, 'executeJavaScript', async (js) => told.push(js));
    };

    it('tells its page a scan began while it covers the page', () => {
      cover();
      ctx.shield.analysisStarted(page);
      assert.deepEqual(told, ['window.oyaShield?.({"phase":"scan"})']);
    });

    it('stays quiet while a person has control, or for a tab in the background', () => {
      cover();
      ctx.shield.analysisStarted({});
      ctx.control.state.interactive = true;
      ctx.shield.analysisStarted(page);
      assert.deepEqual(told, []);
    });

    it('outlines only the visible elements, measured by their selectors in the isolated world', async () => {
      cover();
      const box = { id: 1, type: 'link', x: 4, y: 8, w: 40, h: 20 };
      let measured;
      ctx.world = { worldEval: async (_view, js) => ((measured = js), [box]) };
      const elements = [
        { id: 1, type: 'link', selector: '[data-x="1"]', visible: true },
        { id: 2, type: 'button', selector: '[data-x="2"]', visible: false },
      ];
      await ctx.shield.analysisFinished(page, { ok: true, data: { elements } });
      assert.match(measured, /\[1,"link","\[data-x=\\"1\\"\]"\]/);
      assert.doesNotMatch(measured, /data-x=\\"2/);
      assert.deepEqual(told, [`window.oyaShield?.(${JSON.stringify({ phase: 'found', boxes: [box] })})`]);
    });

    it('outlines nothing when the page cannot be measured', async () => {
      cover();
      ctx.world = { worldEval: async () => Promise.reject(new Error('navigated')) };
      await ctx.shield.analysisFinished(page, { ok: false });
      assert.deepEqual(told, ['window.oyaShield?.({"phase":"found","boxes":[]})']);
    });
  });
});
