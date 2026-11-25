/** Authenticated frame correlation rejects foreign replies without granting a second execution. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { debugRequest, useDebugChannel } from '../../../../src/main/browser-debug/channel.ts';
import { BROWSER_DEBUG as C } from '../../../../src/shared/browser-debug.ts';
/** Correlation callbacks accept untrusted native event and payload values. */
type Receiver = (event: unknown, reply: unknown) => void;
let receive: Receiver;
useDebugChannel({
  on(_name: string, callback: Receiver) {
    receive = callback;
  },
} as never);
test('only the exact native frame and preload document can settle a request', async () => {
  let request: any;
  let settled = false;
  const frame: any = {
    detached: false,
    send(_channel: string, value: unknown) {
      request = value;
    },
  };
  const contents: any = { isDestroyed: () => false, mainFrame: { framesInSubtree: [frame] } };
  const page: any = { webContents: contents };
  const pending = debugRequest(page, frame, 'read', {}, 'document-A').then((value) => {
    settled = true;
    return value;
  });
  const response = { id: request.id, document: 'document-A', ok: true, data: { safe: true } };
  receive({ sender: {}, senderFrame: frame }, response);
  receive({ sender: contents, senderFrame: {} }, response);
  await Promise.resolve();
  assert.equal(settled, false);
  receive({ sender: contents, senderFrame: frame }, response);
  assert.deepEqual((await pending).data, { safe: true });
  receive({ sender: contents, senderFrame: frame }, response);
  assert.equal(settled, true, 'a duplicate reply has no pending request');
});
test('a replacement document cannot authorize a stale operation', async () => {
  let request: any;
  const frame: any = {
    detached: false,
    send(_channel: string, value: unknown) {
      request = value;
    },
  };
  const contents: any = { isDestroyed: () => false, mainFrame: { framesInSubtree: [frame] } };
  const pending = debugRequest({ webContents: contents } as never, frame, 'eval', { expression: '1' }, 'old-document');
  receive({ sender: contents, senderFrame: frame }, { id: request.id, document: 'replacement', ok: true, data: 1 });
  await assert.rejects(pending, /document changed/);
});
test('an expired request is removed and a late response cannot revive it', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  let request: any;
  const frame: any = {
    detached: false,
    send(_channel: string, value: unknown) {
      request = value;
    },
  };
  const contents: any = { isDestroyed: () => false, mainFrame: { framesInSubtree: [frame] } };
  const pending = debugRequest({ webContents: contents } as never, frame, 'eval', {}, 'doc');
  const rejected = assert.rejects(pending, /may have run/);
  t.mock.timers.tick(C.EVAL_MS);
  await rejected;
  receive({ sender: contents, senderFrame: frame }, { id: request.id, document: 'doc', ok: true, data: 1 });
});
