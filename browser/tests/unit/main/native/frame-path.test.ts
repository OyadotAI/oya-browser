/** Native frame paths reject stale topology and preserve the explicit owner chain. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { nativeFramePath } from '../../../../src/main/native/index.ts';
/** Structural frame seam supplies only the native capabilities exercised here. */
function tree() {
  const top = {
    _oyaOwnerFrameToken: () => 'native-token',
    detached: false,
    parent: null,
    frames: [] as unknown[],
    async _executeJavaScriptInOyaWorld() {
      return 'iframe[id=outer]';
    },
  };
  const child = {
    _oyaOwnerFrameToken: () => 'native-token',
    detached: false,
    parent: top,
    frames: [] as unknown[],
    async _executeJavaScriptInOyaWorld() {
      return 'iframe[id=inner]';
    },
  };
  const nested = { _oyaOwnerFrameToken: () => 'nested-token', detached: false, parent: child, frames: [] as unknown[] };
  top.frames.push(child);
  child.frames.push(nested);
  return { top, child, nested };
}
test('top frame has an empty owner path', async () => {
  const { top } = tree();
  assert.deepEqual(await nativeFramePath(top as never, top as never), []);
});
test('nested owner paths retain top-to-leaf order', async () => {
  const { top, nested } = tree();
  assert.deepEqual(await nativeFramePath(top as never, nested as never), ['iframe[id=outer]', 'iframe[id=inner]']);
});
test('refuses frames outside the owned page', async () => {
  const { top } = tree();
  const outside = tree().child;
  await assert.rejects(nativeFramePath(top as never, outside as never), /outside/);
});
test('refuses topology changes during the native DOM read', async () => {
  const { top, child } = tree();
  top._executeJavaScriptInOyaWorld = async () => {
    top.frames = [];
    return 'iframe[id=wrong]';
  };
  await assert.rejects(nativeFramePath(top as never, child as never), /changed/);
});
test('does not invent a locator when owner resolution fails', async () => {
  const { top, child } = tree();
  top._executeJavaScriptInOyaWorld = async () => {
    throw new Error('Frame has no unique stable selector');
  };
  await assert.rejects(nativeFramePath(top as never, child as never), /unique stable selector/);
});
test('refuses detached frames before any execution', async () => {
  const { top, child } = tree();
  child.detached = true;
  await assert.rejects(nativeFramePath(top as never, child as never), /detached/);
});

test('rejects missing native owner capability without evaluating a guessed path', async () => {
  const { top, child } = tree();
  delete (child as Partial<typeof child>)._oyaOwnerFrameToken;
  top._executeJavaScriptInOyaWorld = async () => {
    assert.fail('must not guess');
  };
  await assert.rejects(nativeFramePath(top as never, child as never), /unsupported/);
});
test('rejects a token that changes during owner resolution', async () => {
  const { top, child } = tree();
  top._executeJavaScriptInOyaWorld = async () => {
    child._oyaOwnerFrameToken = () => 'replacement';
    return 'iframe[id=child]';
  };
  await assert.rejects(nativeFramePath(top as never, child as never), /changed/);
});

test('rejects an ancestor token replaced while resolving a deeper owner', async () => {
  const { top, child, nested } = tree();
  child._executeJavaScriptInOyaWorld = async () => {
    child._oyaOwnerFrameToken = () => 'replacement-ancestor';
    return 'iframe[id=inner]';
  };
  await assert.rejects(nativeFramePath(top as never, nested as never), /changed/);
});
test('rejects a deeper token replaced before that hop is evaluated', async () => {
  const { top, child, nested } = tree();
  top._executeJavaScriptInOyaWorld = async () => {
    nested._oyaOwnerFrameToken = () => 'replacement-descendant';
    return 'iframe[id=outer]';
  };
  child._executeJavaScriptInOyaWorld = async () => assert.fail('must not evaluate a replacement');
  await assert.rejects(nativeFramePath(top as never, nested as never), /changed/);
});
