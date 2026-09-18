/**
 * Unit tests for ShellWindow and the application menu: the theme, the dev
 * log's truncation and redaction, the locked shell page, and the page
 * commands that only act for a person in control.
 */
const { describe, it, beforeEach } = require('node:test');
const assert = require('node:assert/strict');
const { ShellWindow } = require('../../../../main/shell/window.cjs');
const { installApplicationMenu } = require('../../../../main/shell/menu.cjs');
const { mainCtx } = require('../../support/main-ctx.cjs');

describe('ShellWindow', () => {
  let ctx;
  beforeEach(() => {
    ctx = mainCtx({ shell: ShellWindow });
  });

  it('follows the system theme unless the person chose one', () => {
    assert.equal(ctx.shell.background(), '#f5f5f2');
    ctx.electron.nativeTheme.shouldUseDarkColors = true;
    assert.equal(ctx.shell.background(), '#1b1e1c');
    ctx.config.values.ui = { theme: 'light' };
    assert.equal(ctx.shell.background(), '#f5f5f2');
  });

  it('drops messages once the window is gone', () => {
    ctx.shell.send('x', 1);
    ctx.shell.create();
    ctx.shell.window.destroyed = true;
    ctx.shell.send('x', 1);
    assert.deepEqual(ctx.shell.window.webContents.messages, []);
  });

  it('truncates long dev-log entries and redacts secrets', () => {
    ctx.shell.create();
    ctx.shell.devLog(
      'in',
      'big',
      Object.fromEntries(Array.from({ length: 40 }, (_, i) => ['n' + i, 'page words '.repeat(40)])),
    );
    ctx.shell.devLog('out', 'auth', { api_key: 'secret-key' });
    const [big, auth] = ctx.shell.window.webContents.sentOn('dev-log');
    assert.equal(big.data.length, 8000 + '\n... (truncated)'.length);
    assert.ok(big.data.endsWith('\n... (truncated)'));
    assert.ok(!auth.data.includes('secret-key'));
  });

  it('opens with the remembered panel width and never navigates away', () => {
    ctx.config.values.ui = { panelWidth: 420 };
    ctx.shell.create();
    assert.equal(ctx.layout.width, 420);
    const event = {
      prevented: false,
      preventDefault() {
        this.prevented = true;
      },
    };
    ctx.shell.window.webContents.emit('will-navigate', event);
    assert.equal(event.prevented, true);
    assert.deepEqual(ctx.shell.window.webContents.openHandler(), { action: 'deny' });
  });

  it('repaints when the system theme changes', () => {
    ctx.shell.create();
    ctx.electron.nativeTheme.shouldUseDarkColors = true;
    ctx.electron.nativeTheme.emit('updated');
    assert.equal(ctx.shell.window.background, '#1b1e1c');
    assert.deepEqual(ctx.shell.window.webContents.sentOn('shell-appearance'), [true]);
  });
});

describe('application menu', () => {
  /** The installed menu's item with this id. */
  const item = (ctx, id) =>
    ctx.electron.Menu.installed.template.flatMap((m) => m.submenu || []).find((i) => i.id === id);

  it('runs page commands only for a person in control', () => {
    const ctx = mainCtx();
    let opened = 0;
    ctx.tabs = { createTab: () => opened++ };
    installApplicationMenu(ctx);
    item(ctx, 'browser-new-tab').click();
    ctx.control.state.interactive = false;
    item(ctx, 'browser-new-tab').click();
    assert.equal(opened, 1);
  });

  it('names the product in the menus', () => {
    const ctx = mainCtx();
    installApplicationMenu(ctx);
    const labels = JSON.stringify(ctx.electron.Menu.installed.template);
    assert.match(labels, /Quit Oya Browser/);
  });
});
