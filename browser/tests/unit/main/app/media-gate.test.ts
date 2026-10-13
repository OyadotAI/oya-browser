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
