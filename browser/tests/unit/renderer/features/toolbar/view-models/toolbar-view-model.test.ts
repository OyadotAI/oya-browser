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
    assert.deepEqual(fake.called('showLibrary'), [['history']]);
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

  it('shows the address the main process reports while nobody is typing', () => {
    const { fake, vm } = toolbar();
    fake.emit('onUrlChanged', 'https://b.test/');
    assert.deepEqual([vm.state.url, vm.state.page], ['https://b.test/', 'https://b.test/']);
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

  it('names an incognito window in its title', (t) => {
    (globalThis as any).document = { documentElement: { dataset: { incognito: 'true' } } };
    t.after(() => delete (globalThis as any).document);
    assert.equal(windowTitle('Example'), 'Example, Oya Browser (Incognito)');
    assert.equal(windowTitle(''), 'Oya Browser (Incognito)');
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

/** Completion changes navigation only after deliberate selection. */
it('submits a selected suggestion and closes the dropdown', async () => {
  const { fake, vm } = toolbar({
    addressSuggestions: [{ url: 'https://example.test/', title: 'Example', source: 'history' }],
  });
  vm.edit('exam');
  await settle();
  vm.completion.move(1);
  vm.submit();
  assert.deepEqual(fake.called('navigate'), [['https://example.test/']]);
  assert.equal(vm.completion.state.open, false);
});
it('dismisses suggestions when the active tab changes', async () => {
  const { fake, vm } = toolbar({
    addressSuggestions: [{ url: 'https://example.test/', title: 'Example', source: 'history' }],
  });
  fake.emit('onTabsUpdated', [active()]);
  vm.edit('exam');
  await settle();
  fake.emit('onTabsUpdated', [active({ id: 2 })]);
  assert.equal(vm.completion.state.open, false);
});

/** What the person types survives a page changing its own address, until they are done with it. */
describe('ToolbarViewModel typing', () => {
  /** A toolbar on tab 1, showing https://a.test/. */
  function onPage(answers = {}) {
    const made = toolbar(answers);
    made.fake.emit('onTabsUpdated', [active()]);
    made.fake.emit('onUrlChanged', 'https://a.test/');
    return made;
  }

  it('keeps what the person typed when the page changes its own address', () => {
    const { fake, vm } = onPage();
    vm.edit('half typed');
    fake.emit('onUrlChanged', 'https://a.test/#t=12');
    assert.deepEqual([vm.state.url, vm.state.page], ['half typed', 'https://a.test/#t=12']);
  });

  it('keeps the typing through the tab list updates the same tab sends', () => {
    const { fake, vm } = onPage();
    vm.edit('half typed');
    fake.emit('onUrlChanged', 'https://a.test/next');
    fake.emit('onTabsUpdated', [active({ loading: true })]);
    assert.equal(vm.state.url, 'half typed');
  });

  it("drops the typing on a tab switch and shows the new tab's address", () => {
    const { fake, vm } = onPage();
    vm.edit('half typed');
    // A switch sends the new tab's address first, then the tab list (tab-lifecycle.ts mountTab).
    fake.emit('onUrlChanged', 'https://b.test/');
    fake.emit('onTabsUpdated', [active({ id: 2 })]);
    assert.equal(vm.state.url, 'https://b.test/');
  });

  it('ends the typing on Enter, so the loaded address shows', () => {
    const { fake, vm } = onPage();
    vm.edit('example.com');
    vm.submit();
    fake.emit('onUrlChanged', 'https://example.com/');
    assert.equal(vm.state.url, 'https://example.com/');
  });

  it('ends the typing when a suggestion is chosen', () => {
    const { fake, vm } = onPage();
    vm.edit('exam');
    vm.submit('https://example.test/');
    fake.emit('onUrlChanged', 'https://example.test/');
    assert.equal(vm.state.url, 'https://example.test/');
  });

  it('keeps the typed text on blur, until the next address change', () => {
    const { fake, vm } = onPage();
    vm.edit('half typed');
    vm.blurred();
    assert.equal(vm.state.url, 'half typed');
    fake.emit('onUrlChanged', 'https://a.test/next');
    assert.equal(vm.state.url, 'https://a.test/next');
  });

  it('closes open suggestions on Escape and keeps the typing', async () => {
    const { vm } = onPage({ addressSuggestions: [{ url: 'https://example.test/', title: 'E', source: 'history' }] });
    vm.edit('exam');
    await settle();
    vm.escape();
    assert.deepEqual([vm.state.url, vm.completion.state.open], ['exam', false]);
  });

  it("puts the page's address back, selected, on Escape with no suggestions open", () => {
    const { fake, vm } = onPage();
    vm.edit('half typed');
    fake.emit('onUrlChanged', 'https://a.test/next');
    const before = vm.state.focusRequest;
    vm.escape();
    assert.deepEqual([vm.state.url, vm.state.focusRequest], ['https://a.test/next', before + 1]);
  });

  it("keeps the address Escape restored, so it stays selected, through the page's next change", () => {
    const { fake, vm } = onPage();
    vm.focused();
    vm.edit('half typed');
    vm.escape();
    fake.emit('onUrlChanged', 'https://a.test/later');
    assert.deepEqual([vm.state.url, vm.state.page], ['https://a.test/', 'https://a.test/later']);
  });

  it('keeps a focused field and its suggestions when the page changes its address', async () => {
    const { fake, vm } = onPage({ addressSuggestions: [{ url: 'https://a.test/', title: 'A', source: 'history' }] });
    vm.focused();
    await vm.completion.search(vm.state.url);
    fake.emit('onUrlChanged', 'https://a.test/next');
    assert.deepEqual([vm.state.url, vm.completion.state.open], ['https://a.test/', true]);
  });

  it("shows the page's address on blur when the field was only looked at", () => {
    const { fake, vm } = onPage();
    vm.focused();
    fake.emit('onUrlChanged', 'https://a.test/next');
    vm.blurred();
    assert.equal(vm.state.url, 'https://a.test/next');
  });

  it('clears a completion left over when the address is replaced', async () => {
    const { fake, vm } = onPage({
      addressSuggestions: [{ url: 'https://example.test/', title: 'E', source: 'history' }],
    });
    vm.edit('exam', true);
    await settle();
    vm.blurred();
    fake.emit('onUrlChanged', 'https://a.test/next');
    assert.deepEqual([vm.state.url, vm.completion.state.inline], ['https://a.test/next', '']);
  });

  it('does not let the starting status overwrite typing', async () => {
    const { vm } = toolbar({ getStatus: { url: 'https://a.test/' } });
    vm.edit('typed early');
    await settle();
    assert.deepEqual([vm.state.url, vm.state.page], ['typed early', 'https://a.test/']);
  });

  it('does not let a late starting status overwrite a newer page', async () => {
    const { fake, vm } = toolbar({ getStatus: { url: 'https://old.test/' } });
    fake.emit('onUrlChanged', 'https://new.test/');
    await settle();
    assert.equal(vm.state.page, 'https://new.test/');
  });
});
