/**
 * Unit tests for the navigation toolbar: the address follows the main
 * process and what the person types, Enter loads it, the buttons follow the
 * active tab, and the window takes the page's title.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  ToolbarViewModel,
  navLook,
  windowTitle,
  askClass,
} from '../../../../../../src/renderer/features/toolbar/view-models/toolbar-view-model.ts';
import { fakeBridge } from '../../../support/bridge.ts';

/** Lets the fake bridge's promises settle. */
const settle = () => new Promise<void>((resolve) => setImmediate(resolve));

/** A toolbar over a fake bridge. */
function toolbar(answers = {}) {
  const fake = fakeBridge(answers);
  return { fake, vm: new ToolbarViewModel(fake.bridge) };
}

/** An active tab with `extra`. */
const active = (extra = {}) => ({ id: 1, title: '', url: '', home: false, favicon: null, active: true, ...extra });

describe('ToolbarViewModel', () => {
  it('focuses each newly selected start page, not each background loading update', () => {
    const { fake, vm } = toolbar();
    fake.emit('onTabsUpdated', [active({ home: true })]);
    assert.equal(vm.state.focusRequest, 1);
    fake.emit('onTabsUpdated', [active({ home: true })]);
    assert.equal(vm.state.focusRequest, 1);
    fake.emit('onTabsUpdated', [active({ home: true, id: 2 })]);
    assert.equal(vm.state.focusRequest, 2);
    fake.emit('onTabsUpdated', [active({ id: 3 })]);
    assert.equal(vm.state.focusRequest, 2);
  });
  it('opens the native library through the shell bridge', () => {
    const { fake, vm } = toolbar();
    vm.showLibrary();
    assert.deepEqual(fake.called('showLibrary'), [[]]);
  });

  it("starts from the main process's address", async () => {
    const { vm } = toolbar({ getStatus: { url: 'https://a.test/' } });
    await settle();
    assert.equal(vm.state.url, 'https://a.test/');
  });

  it('starts empty when the status has no address', async () => {
    const { vm } = toolbar({ getStatus: undefined });
    await settle();
    assert.equal(vm.state.url, '');
  });

  it('shows the address the main process reports', () => {
    const { fake, vm } = toolbar();
    vm.edit('half typed');
    fake.emit('onUrlChanged', 'https://b.test/');
    assert.equal(vm.state.url, 'https://b.test/');
  });

  it('loads what the address bar says, trimmed, on Enter', () => {
    const { fake, vm } = toolbar();
    vm.edit('  example.com ');
    vm.submit();
    assert.deepEqual(fake.called('navigate'), [['example.com']]);
  });

  it('goes back, forward and reloads', () => {
    const { fake, vm } = toolbar();
    vm.back();
    vm.forward();
    vm.reload();
    assert.deepEqual(
      ['goBack', 'goForward', 'reload'].map((name) => fake.called(name).length),
      [1, 1, 1],
    );
  });

  it("follows the active tab's history and loading", () => {
    const { fake, vm } = toolbar();
    fake.emit('onTabsUpdated', [active({ loading: true, canGoBack: true }), { ...active(), id: 2, active: false }]);
    assert.deepEqual(vm.state.nav, { loading: true, error: '', canGoBack: true, canGoForward: false });
  });

  it('has nowhere to go with no active tab', () => {
    const { fake, vm } = toolbar();
    fake.emit('onTabsUpdated', [{ ...active(), active: false, canGoBack: true }]);
    assert.equal(vm.state.nav.canGoBack, false);
  });

  it('reads the load error by the name the main process sends', () => {
    const { fake, vm } = toolbar();
    fake.emit('onTabsUpdated', [active({ loadError: 'Refused' })]);
    assert.equal(vm.state.nav.error, 'Refused');
  });

  it('asks the view to focus the address bar each time', () => {
    const { vm } = toolbar();
    vm.focusAddress();
    vm.focusAddress();
    assert.equal(vm.state.focusRequest, 2);
  });

  it('keeps the page title for the window', () => {
    const { fake, vm } = toolbar();
    fake.emit('onTitleChanged', 'Example');
    assert.equal(windowTitle(vm.state.title), 'Example, Oya Browser');
    assert.equal(windowTitle(''), 'Oya Browser');
  });

  it('stops following the main process on dispose', () => {
    const { fake, vm } = toolbar();
    vm.dispose();
    assert.equal(
      fake.listeners('onUrlChanged') + fake.listeners('onTabsUpdated') + fake.listeners('onTitleChanged'),
      0,
    );
  });
});

describe('navLook', () => {
  it('turns Reload into Stop and says Loading while the page loads', () => {
    const look = navLook({ loading: true, error: '', canGoBack: false, canGoForward: false });
    assert.deepEqual(look, {
      status: 'Loading…',
      statusLabel: 'Loading…',
      statusTitle: '',
      reloadIcon: 'close',
      reloadLabel: 'Stop loading',
    });
  });

  it('says the load failed, with why as its name and tooltip', () => {
    const look = navLook({ loading: false, error: 'DNS failed', canGoBack: false, canGoForward: false });
    assert.deepEqual([look.status, look.statusLabel, look.statusTitle], ['Load failed', 'DNS failed', 'DNS failed']);
  });

  it('is quiet at rest', () => {
    const look = navLook({ loading: false, error: '', canGoBack: false, canGoForward: false });
    assert.deepEqual([look.status, look.reloadIcon, look.reloadLabel], ['', 'reload', 'Reload page']);
  });
});

describe('the Ask button', () => {
  it('shows the panel open and a recording dot', () => {
    assert.equal(askClass(false, false), 'dev-btn agent-button');
    assert.equal(askClass(true, true), 'dev-btn agent-button active recording');
  });
});
