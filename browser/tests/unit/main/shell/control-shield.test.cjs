/**
 * Unit tests for ControlShield: the transparent shield over the page while an
 * agent drives, popups and menu items fenced, and the human-control guard.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { ControlShield } = require('../../../../main/shell/control-shield.cjs');
const { mainCtx, FakeBrowserView } = require('../../support/main-ctx.cjs');
const { SHIELD_TRACK_MS, SHIELD_TRACK_FOR_MS } = require('../../../../main/shell/constants.cjs');

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
    const agent = { interactive: false, mode: 'agent', mine: false };
    ctx.shield.controlChanged(agent);
    assert.equal(popup.enabled, false);
    assert.equal(item.enabled, false);
    assert.equal(lost.mock.callCount(), 1);
    assert.deepEqual(ctx.shell.sentOn('control-state'), [agent]);
  });

  it('leaves a sign-in popup usable when the hold lapses or the socket drops: nobody else is driving', () => {
    const popup = Object.assign(new EventEmitter(), { setEnabled: mock.fn(), isDestroyed: () => false });
    ctx.shield.adoptPopup(popup);
    for (const state of [
      { mode: 'paused', mine: false },
      { mode: 'disconnected' },
      { mode: 'human', mine: true, busy: true },
    ])
      ctx.shield.controlChanged({ interactive: false, ...state });
    assert.deepEqual(
      popup.setEnabled.mock.calls.map((c) => c.arguments[0]),
      [true, true, true, true],
    );
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

    it('scales outlines by the page zoom, since the shield itself is not zoomed', async () => {
      cover();
      page.webContents.zoomFactor = 1.25;
      ctx.world = { worldEval: async () => [{ id: 1, type: 'link', x: 8, y: 16, w: 40, h: 20 }] };
      await ctx.shield.analysisFinished(page, { data: { elements: [] } });
      ctx.shield.stopTracking();
      assert.deepEqual(told, [
        `window.oyaShield?.(${JSON.stringify({ phase: 'found', boxes: [{ id: 1, type: 'link', x: 10, y: 20, w: 50, h: 25 }] })})`,
      ]);
    });

    describe('following the outlines', () => {
      beforeEach(() => mock.timers.enable({ apis: ['setTimeout', 'Date'] }));
      afterEach(() => mock.timers.reset());

      /** Lets the next re-measure run and its answer arrive. */
      const step = async () => {
        mock.timers.tick(SHIELD_TRACK_MS);
        await new Promise((resolve) => setImmediate(resolve));
      };
      /** Finds one element whose y each measurement reads from `where`. */
      const found = async (where) => {
        cover();
        ctx.world = { worldEval: async () => [{ id: 1, type: 'link', x: 0, y: where.y, w: 5, h: 5 }] };
        await ctx.shield.analysisFinished(page, { data: { elements: [] } });
      };
      /** The phases told so far. */
      const phases = () => told.map((js) => JSON.parse(js.slice('window.oyaShield?.('.length, -1)).phase);

      it('re-measures and tells the page where the elements moved', async () => {
        const where = { y: 40 };
        await found(where);
        where.y = 10;
        await step();
        assert.deepEqual(phases(), ['found', 'move']);
        assert.match(told[1], /"y":10/);
      });

      it('stops on a new scan, and when the shield comes off', async () => {
        await found({ y: 0 });
        ctx.shield.analysisStarted(page);
        await step();
        assert.deepEqual(phases(), ['found', 'scan']);
        await found({ y: 0 });
        ctx.control.state.interactive = true;
        ctx.shield.sync();
        await step();
        assert.deepEqual(phases(), ['found']);
      });

      it('stops once the show is over', async () => {
        await found({ y: 0 });
        mock.timers.tick(SHIELD_TRACK_FOR_MS);
        await step();
        const before = told.length;
        await step();
        await step();
        assert.equal(told.length, before);
      });

      it('skips a re-measure the page could not answer, and keeps following', async () => {
        await found({ y: 0 });
        ctx.world = { worldEval: async () => Promise.reject(new Error('navigated')) };
        await step();
        assert.deepEqual(phases(), ['found']);
        ctx.world = { worldEval: async () => [] };
        await step();
        assert.deepEqual(phases(), ['found', 'move']);
      });
    });

    it('outlines nothing when the page cannot be measured', async () => {
      cover();
      ctx.world = { worldEval: async () => Promise.reject(new Error('navigated')) };
      await ctx.shield.analysisFinished(page, { ok: false });
      assert.deepEqual(told, ['window.oyaShield?.({"phase":"found","boxes":[]})']);
    });
  });
});
