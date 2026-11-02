/** Capture choices remain scoped to the requesting profile and live human-owned sources. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { ShareSources, webHost } from '../../../../src/main/screen-share/sources.ts';

/** Native thumbnail seam contains no actual desktop pixels. */
const image = { isEmpty: () => false, resize: () => image, toDataURL: () => 'data:image/png;base64,test' };
/** A real EventEmitter exercises exact navigation-listener lifetime. */
function tab(id, session) {
  const contents = Object.assign(new EventEmitter(), {
    session,
    mainFrame: { id },
    isDestroyed: () => false,
    capturePage: async () => image,
  });
  return { id, title: `Tab ${id}`, url: `https://tab.test/${id}?secret=1`, view: { webContents: contents } };
}
/** Two owned tabs, one foreign profile, one claimed tab, and native screen/window options. */
function fixture(platform = 'darwin') {
  const session = {};
  const tabs = [tab(1, session), tab(2, session), tab(3, {}), tab(4, session)];
  const desktop = [
    { id: 'window:42:0', name: 'Editor', thumbnail: image },
    { id: 'screen:0:0', name: 'Main display', thumbnail: image },
  ];
  const deps = {
    tabs: { list: tabs },
    agentTab: { humanMay: (id) => id !== 4 },
    electron: {
      desktopCapturer: { getSources: async () => desktop },
      systemPreferences: { getMediaAccessStatus: () => 'granted' },
    },
  };
  const sources = new ShareSources(deps as any, tabs[0].view.webContents as any, platform);
  return { sources, deps, tabs, desktop };
}

it('lists same-profile human tabs, windows and screens with opaque tokens', async () => {
  const f = fixture();
  try {
    const options = await f.sources.read();
    assert.deepEqual(
      options.map(({ kind }) => kind),
      ['tab', 'tab', 'window', 'screen'],
    );
    assert.equal(options[0].detail, 'tab.test');
    assert.equal(f.sources.resolve(options[1].id), f.tabs[1].view.webContents.mainFrame);
    assert.equal(f.sources.resolve(options[2].id), f.desktop[0]);
    assert.equal(f.sources.resolve(options[3].id), f.desktop[1]);
    assert.equal(f.sources.resolve('screen:0:0'), null);
    assert.equal(f.sources.resolve(null), null);
  } finally {
    f.sources.dispose();
  }
});

it('rejects a tab closed, navigated or claimed after its preview', async () => {
  for (const change of [
    (f) => f.tabs[0].view.webContents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: false }),
    (f) => {
      f.deps.agentTab.humanMay = () => false;
    },
    (f) => {
      f.deps.tabs.list = [];
    },
  ]) {
    const f = fixture();
    const [option] = await f.sources.read();
    change(f);
    assert.equal(f.sources.resolve(option.id), null);
    f.sources.dispose();
    assert.equal(f.tabs[0].view.webContents.listenerCount('did-start-navigation'), 0);
  }
});

it('keeps tab choices when OS source enumeration fails', async () => {
  const f = fixture();
  f.deps.electron.desktopCapturer.getSources = async () => {
    throw Error('OS denied');
  };
  try {
    const options = await f.sources.read();
    assert.deepEqual(
      options.map(({ kind }) => kind),
      ['tab', 'tab'],
    );
    assert.match(f.sources.desktopError, /screen recording/);
  } finally {
    f.sources.dispose();
  }
});

it('a missing preview does not remove a valid tab, and disposal revokes all tokens', async () => {
  const f = fixture();
  f.tabs[0].view.webContents.capturePage = async () => {
    throw Error('Renderer asleep');
  };
  const [option] = await f.sources.read();
  assert.equal(option.thumbnail, '');
  assert.ok(f.sources.resolve(option.id));
  f.sources.dispose();
  assert.equal(f.sources.resolve(option.id), null);
});

it('asks the OS for sources before checking Screen Recording access', async () => {
  const f = fixture();
  let status = 'denied';
  f.deps.electron.systemPreferences.getMediaAccessStatus = () => status;
  f.deps.electron.desktopCapturer.getSources = async () => {
    status = 'granted';
    return f.desktop;
  };
  try {
    const options = await f.sources.read();
    assert.equal(options.length, 4);
    assert.equal(f.sources.canOpenSettings, false);
    status = 'denied';
    assert.equal(f.sources.resolve(options[2].id), null, 'permission revoked before sharing');
  } finally {
    f.sources.dispose();
  }
});

it('keeps tabs but offers Settings when macOS returns unusable desktop sources', async () => {
  const f = fixture();
  f.deps.electron.systemPreferences.getMediaAccessStatus = () => 'denied';
  try {
    const options = await f.sources.read();
    assert.deepEqual(
      options.map(({ kind }) => kind),
      ['tab', 'tab'],
    );
    assert.equal(f.sources.canOpenSettings, true);
    assert.ok(f.sources.desktopError);
  } finally {
    f.sources.dispose();
  }
});

it('empty tab and desktop captures become placeholders without removing valid choices', async () => {
  const f = fixture('linux');
  const empty = {
    ...image,
    isEmpty: () => true,
    toDataURL: () => {
      throw Error('Empty image must not be encoded');
    },
  };
  f.tabs[0].view.webContents.capturePage = async () => empty;
  f.desktop[0].thumbnail = empty;
  try {
    const options = await f.sources.read();
    assert.equal(options[0].thumbnail, '');
    assert.equal(options[2].thumbnail, '');
    assert.equal(f.sources.resolve(options[2].id), f.desktop[0]);
  } finally {
    f.sources.dispose();
  }
});

it("a tab's detail is its web host, never its path, query, credentials or a local page", () => {
  assert.equal(webHost('https://user:pw@call.test:8443/room?secret=1'), 'call.test:8443');
  assert.equal(webHost('http://a.test/'), 'a.test');
  assert.equal(webHost('file:///Users/me/notes.html'), null);
  assert.equal(webHost('oya://home'), null);
  assert.equal(webHost('not a url'), null);
});
