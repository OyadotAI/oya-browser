/**
 * Unit tests for src/main/app/media-gate.ts's call tracking: a document Oya
 * gave a device counts as in a call until that document goes, which is what
 * the close and quit dialogs (call-guard.ts) ask about.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { fixture, details } from '../../support/media-fixture.ts';

/** A new top-level document starting, as Electron describes a reload. */
const reload = { isMainFrame: true, isSameDocument: false };

/** Native ownership stays stable while the application's selected window can change independently. */
function owners() {
  const f = fixture();
  const window = { isDestroyed: () => false };
  const owner = { tabs: f.deps.tabs, shell: { window } };
  let current = owner;
  (f.deps as any).windows = { ownerOfContents: (contents) => (contents === f.contents ? current : undefined) };
  return {
    ...f,
    owner,
    window,
    move: (scope) => {
      current = scope;
    },
  };
}

it('uses the requester window instead of the globally selected window for permission and prompt parenting', async () => {
  const f = owners(),
    parents: unknown[] = [];
  f.deps.tabs = { activeTabId: 2, getActiveView: () => ({ webContents: {} }) } as any;
  f.deps.electron.dialog.showMessageBox = async (...args) => {
    parents.push(args[0]);
    return { response: 1 };
  };
  assert.equal(await f.service.request(f.contents as any, details as any), true);
  assert.deepEqual(parents, [f.window]);
});

it('a changed global target does not revoke a request whose own window remains valid', async () => {
  const f = owners(),
    pending = Promise.withResolvers<any>();
  f.deps.electron.dialog.showMessageBox = () => pending.promise;
  const request = f.service.request(f.contents as any, details as any);
  f.deps.tabs = { activeTabId: 2, getActiveView: () => null } as any;
  pending.resolve({ response: 1 });
  assert.equal(await request, true);
});

it('a transferred requester cannot persist or receive an approval started in another window', async () => {
  const f = owners(),
    pending = Promise.withResolvers<any>();
  f.deps.electron.dialog.showMessageBox = () => pending.promise;
  const request = f.service.request(f.contents as any, details as any);
  f.move({ ...f.owner, shell: { window: { isDestroyed: () => false } } });
  pending.resolve({ response: 1 });
  assert.equal(await request, false);
  assert.equal(f.deps.sitePermissions.has('persist:one', 'https://call.test', 'audio'), false);
});

it('unregistered and private requesters cannot reuse normal profile permissions', async () => {
  for (const privateWindow of [false, true]) {
    const f = owners();
    f.deps.sitePermissions.grant('persist:one', 'https://call.test', ['audio']);
    f.move(privateWindow ? { ...f.owner, tabs: { ...f.owner.tabs, privateSession: {} } } : undefined);
    assert.equal(f.check(), false);
    assert.equal(await f.service.request(f.contents as any, details as any), false);
    assert.deepEqual(f.devices, []);
  }
});

it('duplicate decisions match only the exact document, profile, origin and unordered device set', () => {
  const f = fixture();
  const key = (origin = 'https://call.test', devices = ['video', 'audio']) =>
    f.service.gate.decisionKey(f.contents as any, origin, devices);
  const original = key();
  assert.equal(key('https://call.test', ['audio', 'video']), original);
  assert.notEqual(key('https://elsewhere.test'), original);
  assert.notEqual(key('https://call.test', ['audio']), original);
  f.contents.emit('did-start-navigation', reload);
  assert.notEqual(key(), original);
  const reloaded = key();
  f.deps.persona.partitionName = () => 'persist:two';
  assert.notEqual(key(), reloaded);
});

/** A reload: the navigation starts, then the new document commits. */
const navigate = (contents) => {
  contents.emit('did-start-navigation', reload);
  contents.emit('did-navigate');
};

it('a document granted a device is in a call', async () => {
  const f = fixture();
  assert.equal(f.service.gate.inCall(f.contents as any), false);
  await f.service.request(f.contents as any, details as any);
  assert.equal(f.service.gate.inCall(f.contents as any), true);
});

it('a refused or cancelled request is not a call', async () => {
  const f = fixture();
  f.state.response = 0;
  await f.service.request(f.contents as any, details as any);
  assert.equal(f.service.gate.inCall(f.contents as any), false);
  f.state.response = 1;
  f.state.os = false;
  await f.service.request(f.contents as any, details as any);
  assert.equal(f.service.gate.inCall(f.contents as any), false, 'macOS refused the device');
});

it('a new top document ends the call; a pushState does not', async () => {
  const f = fixture();
  await f.service.request(f.contents as any, details as any);
  f.contents.emit('did-start-navigation', { isMainFrame: true, isSameDocument: true });
  assert.equal(f.service.gate.inCall(f.contents as any), true);
  navigate(f.contents);
  assert.equal(f.service.gate.inCall(f.contents as any), false);
});

it('a navigation that never commits, like a download in the meeting tab, keeps the call', async () => {
  const f = fixture();
  await f.service.request(f.contents as any, details as any);
  f.contents.emit('did-start-navigation', reload);
  assert.equal(f.service.gate.inCall(f.contents as any), true);
});

it('a remembered grant marks the rejoined call live', async () => {
  const f = fixture();
  await f.service.request(f.contents as any, details as any);
  navigate(f.contents);
  await f.service.request(f.contents as any, details as any);
  assert.equal(f.prompts.length, 1, 'remembered');
  assert.equal(f.service.gate.inCall(f.contents as any), true);
});

it('a tab an agent took is still in a call', async () => {
  const f = fixture();
  await f.service.request(f.contents as any, details as any);
  f.state.interactive = false;
  assert.equal(f.service.gate.inCall(f.contents as any), true);
});

it('a destroyed tab is not in a call', async () => {
  const f = fixture();
  await f.service.request(f.contents as any, details as any);
  f.state.destroyed = true;
  assert.equal(f.service.gate.inCall(f.contents as any), false);
});

it('a crashed tab is not in a call', async () => {
  const f = fixture();
  await f.service.request(f.contents as any, details as any);
  f.state.crashed = true;
  assert.equal(f.service.gate.inCall(f.contents as any), false);
});
