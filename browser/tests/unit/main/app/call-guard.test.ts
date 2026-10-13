/**
 * Unit tests for src/main/app/call-guard.ts: a window close or a quit that
 * would end a call asks first, with Cancel the default; a quit nobody needs
 * to be asked about is committed at once; the OS shutting down is never held.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { CallGuard } from '../../../../src/main/app/call-guard.ts';
import { SHUTDOWN_GRACE_MS } from '../../../../src/main/app/constants.ts';
import { sitesLabel, endCallPrompt } from '../../../../src/main/app/media-prompts.ts';

/** A tab whose document is (or is not) holding a device. */
const tab = (id: number, url: string, live = true) => ({
  id,
  view: { webContents: { live, getURL: () => url, isDestroyed: () => false } },
});

/** A guard over fake tabs, one window, and a dialog that answers `response`. */
function fixture(tabs: any[], platform = 'darwin') {
  const window = {
    closes: 0,
    shown: 0,
    isDestroyed: () => false,
    close() {
      this.closes++;
    },
    show() {
      this.shown++;
    },
  };
  const asked: any[] = [];
  const state = { response: 0, quits: 0, release: null as null | (() => void) };
  const electron = {
    app: { quit: () => state.quits++ },
    dialog: {
      showMessageBox: (_parent: unknown, options: any) =>
        new Promise((resolve) => {
          asked.push(options);
          state.release = () => resolve({ response: options.signal.aborted ? 0 : state.response });
          options.signal.addEventListener('abort', state.release);
        }),
    },
  };
  const scope = {
    shell: { window },
    tabs: {
      active: 0,
      activateTab(id: number) {
        this.active = id;
      },
    },
  };
  const windows = { allTabs: () => tabs, owner: () => scope };
  const gate = { inCall: (contents: any) => contents.live };
  const guard = new CallGuard({ electron, windows, mediaPermissions: { gate } } as any, platform);
  return { guard, window, asked, state, scope };
}

/** A before-quit or close event that records preventDefault. */
const event = () => ({
  held: false,
  preventDefault() {
    this.held = true;
  },
});

/** Lets the dialog answer and the follow-up run. */
const answer = async (f: ReturnType<typeof fixture>) => {
  await new Promise((r) => setImmediate(r));
  f.state.release?.();
  await new Promise((r) => setImmediate(r));
};

it('names sites, not tabs: one, two, then the first and a count', () => {
  assert.equal(sitesLabel(['meet.google.com']), 'meet.google.com');
  assert.equal(sitesLabel(['meet.google.com', 'zoom.us']), 'meet.google.com and zoom.us');
  assert.equal(sitesLabel(['meet.google.com', 'zoom.us', 'teams.live.com']), 'meet.google.com and 2 other sites');
});

it('says is for one site and are for more, with Cancel the default and Escape', () => {
  assert.match(endCallPrompt('close', ['a.test']).detail!, /^a\.test is using your camera/);
  assert.match(endCallPrompt('quit', ['a.test', 'b.test']).detail!, /^a\.test and b\.test are using/);
  const prompt = endCallPrompt('quit', ['a.test']);
  assert.deepEqual([prompt.buttons, prompt.defaultId, prompt.cancelId], [['Cancel', 'Quit'], 0, 0]);
});

it('counts each site once and only the tabs whose document holds a device', () => {
  const tabs = [
    tab(1, 'https://meet.test/a?pwd=x'),
    tab(2, 'https://meet.test/b'),
    tab(3, 'https://idle.test/', false),
  ];
  assert.deepEqual(fixture(tabs).guard.sites(tabs as any), ['meet.test']);
});

it('a close with no call goes through without a dialog', () => {
  const f = fixture([tab(1, 'https://idle.test/', false)]);
  const e = event();
  assert.equal(f.guard.holdClose(f.window as any, [tab(1, 'https://idle.test/', false)] as any, e as any), false);
  assert.equal(e.held, false);
  assert.deepEqual(f.asked, []);
});

it('a close that would end a call is held; Cancel keeps the window', async () => {
  const tabs = [tab(1, 'https://meet.test/room')];
  const f = fixture(tabs);
  const e = event();
  assert.equal(f.guard.holdClose(f.window as any, tabs as any, e as any), true);
  assert.equal(e.held, true);
  await answer(f);
  assert.equal(f.asked[0].message, 'Close this window and end your call?');
  assert.equal(f.window.closes, 0);
});

it('Close Window closes the window again, and that close goes through', async () => {
  const tabs = [tab(1, 'https://meet.test/room')];
  const f = fixture(tabs);
  f.state.response = 1;
  let reentered: boolean | undefined;
  f.window.close = () => (reentered = f.guard.holdClose(f.window as any, tabs as any, event() as any));
  f.guard.holdClose(f.window as any, tabs as any, event() as any);
  await answer(f);
  assert.equal(reentered, false);
});

it('a quit with no call is committed at once, not held', () => {
  const f = fixture([tab(1, 'https://idle.test/', false)]);
  const e = event();
  assert.equal(f.guard.holdQuit(e as any), false);
  assert.equal(f.guard.committed, true);
});

it('a quit with a call brings it forward and asks; Cancel commits nothing', async () => {
  const f = fixture([tab(1, 'https://idle.test/', false), tab(7, 'https://meet.test/room')]);
  assert.equal(f.guard.holdQuit(event() as any), true);
  await answer(f);
  assert.equal(f.asked[0].message, 'Quit Oya Browser and end your call?');
  assert.deepEqual([f.window.shown, f.scope.tabs.active], [1, 7]);
  assert.deepEqual([f.guard.committed, f.state.quits], [false, 0]);
});

it('Quit commits and quits again', async () => {
  const f = fixture([tab(1, 'https://meet.test/room')]);
  f.state.response = 1;
  f.guard.holdQuit(event() as any);
  await answer(f);
  assert.deepEqual([f.guard.committed, f.state.quits], [true, 1]);
});

it('a second quit while the dialog is up asks nothing more', async () => {
  const f = fixture([tab(1, 'https://meet.test/room')]);
  f.guard.holdQuit(event() as any);
  assert.equal(f.guard.holdQuit(event() as any), true, 'still held');
  await new Promise((r) => setImmediate(r));
  assert.equal(f.asked.length, 1);
});

it('the OS shutting down dismisses an open dialog and nothing after it is held', async () => {
  const f = fixture([tab(1, 'https://meet.test/room')]);
  f.guard.holdQuit(event() as any);
  await new Promise((r) => setImmediate(r));
  f.guard.shutdown();
  await new Promise((r) => setImmediate(r));
  assert.equal(f.asked[0].signal.aborted, true);
  assert.equal(f.guard.holdQuit(event() as any), false);
  assert.equal(f.guard.holdClose(f.window as any, [tab(1, 'https://meet.test/')] as any, event() as any), false);
});

it('asks nothing off macOS', () => {
  const tabs = [tab(1, 'https://meet.test/room')];
  const f = fixture(tabs, 'win32');
  assert.equal(f.guard.holdClose(f.window as any, tabs as any, event() as any), false);
  assert.equal(f.guard.holdQuit(event() as any), false);
});

it('a confirmed close that something else holds asks again next time', async () => {
  const tabs = [tab(1, 'https://meet.test/room')];
  const f = fixture(tabs);
  f.state.response = 1;
  f.guard.holdClose(f.window as any, tabs as any, event() as any);
  await answer(f);
  assert.equal(f.window.closes, 1, 'the fake window stays open, as if an agent held it');
  assert.equal(f.guard.holdClose(f.window as any, tabs as any, event() as any), true);
});

it('a logout another app cancels lapses, and calls are guarded again', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const f = fixture([tab(1, 'https://meet.test/room')]);
  f.guard.shutdown();
  assert.equal(f.guard.committed, true);
  t.mock.timers.tick(SHUTDOWN_GRACE_MS);
  assert.equal(f.guard.committed, false);
  assert.equal(f.guard.holdQuit(event() as any), true);
});

it('a quit that arrives during the logout is committed for good', (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const f = fixture([tab(1, 'https://meet.test/room')]);
  f.guard.shutdown();
  assert.equal(f.guard.holdQuit(event() as any), false);
  t.mock.timers.tick(SHUTDOWN_GRACE_MS);
  assert.equal(f.guard.committed, true);
});
