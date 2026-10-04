/**
 * Unit tests for src/main/recording/frame-sessions.ts: a tab's cross-site iframe
 * sessions, kept from the moment the tab exists, and ports that talk to one.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { trackFrameSessions, framePorts } from '../../../../src/main/recording/frame-sessions.ts';

/** A view whose debugger emits messages and records the commands sent to it. */
function fakeView(): any {
  const dbg: any = Object.assign(new EventEmitter(), { sent: [] });
  dbg.sendCommand = async (method: string, params: any, sessionId: string) => dbg.sent.push([method, sessionId]);
  const emit = (method: string, params: any, sessionId?: string) => dbg.emit('message', {}, method, params, sessionId);
  return { webContents: { debugger: dbg }, emit };
}

/** An iframe target attaching as `sessionId`. */
const attach = (sessionId: string, targetId: string, type = 'iframe') => ({
  sessionId,
  targetInfo: { targetId, type },
});

describe('frame sessions', () => {
  it('lists the iframes attached since the tab was made, and forgets detached ones', () => {
    const view = fakeView();
    trackFrameSessions(view);
    view.emit('Target.attachedToTarget', attach('s1', 'f1'));
    view.emit('Target.attachedToTarget', attach('s2', 'f2'));
    view.emit('Target.attachedToTarget', attach('w1', 'w1', 'worker'));
    view.emit('Target.detachedFromTarget', { sessionId: 's2' });
    assert.deepEqual(framePorts(view)!.list(), [{ sessionId: 's1', frameId: 'f1' }]);
  });

  it('tells a watcher of each iframe that attaches until it stops watching', () => {
    const view = fakeView();
    trackFrameSessions(view);
    const heard: string[] = [];
    const stop = framePorts(view)!.watch((sessionId: string) => heard.push(sessionId));
    view.emit('Target.attachedToTarget', attach('s1', 'f1'));
    stop();
    view.emit('Target.attachedToTarget', attach('s2', 'f2'));
    assert.deepEqual(heard, ['s1']);
  });

  it('gives a port that sends to one session and hears only its events', async () => {
    const view = fakeView();
    trackFrameSessions(view);
    const port = framePorts(view)!.port('s1');
    const heard: string[] = [];
    port.on('Runtime.bindingCalled', (params: any) => heard.push(params.name));
    view.emit('Runtime.bindingCalled', { name: 'mine' }, 's1');
    view.emit('Runtime.bindingCalled', { name: 'other' }, 's2');
    view.emit('Runtime.bindingCalled', { name: 'page' });
    await port.send('Runtime.enable');
    assert.deepEqual(heard, ['mine']);
    assert.deepEqual(view.webContents.debugger.sent, [['Runtime.enable', 's1']]);
  });

  it('has nothing to offer for a view it never tracked', () => {
    assert.equal(framePorts(fakeView()), null);
  });
});
