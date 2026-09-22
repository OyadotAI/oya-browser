/**
 * Unit tests for scripts/recording/remote-frames.cjs: cross-site iframes armed
 * through their own sessions, their steps placed in the page, and the ones
 * that could not be armed flagged.
 */
const { describe, it, mock } = require('node:test');
const assert = require('node:assert/strict');
const { RemoteFrames } = require('../../../../scripts/recording/remote-frames.cjs');

/** A page channel that keeps what it receives; `path` answers its frame path, or throws when null. */
function parentChannel(path = ['iframe[id="pay"]']) {
  const received = [];
  const send = async (method) => {
    if (method === 'DOM.getFrameOwner') return { backendNodeId: 1 };
    return { node: { localName: 'iframe', attributes: ['name', 'widget'] } };
  };
  const framePath = async () => {
    if (!path) throw new Error('Frame detached');
    return path;
  };
  return { received, send, framePath, receive: (out) => received.push(out) };
}

/** Iframe sessions: those attached now, and a way to attach one later. */
function frameList(now = []) {
  const watchers = [];
  return {
    list: () => now,
    watch: (fn) => (watchers.push(fn), () => watchers.splice(0)),
    port: (sessionId) => ({ sessionId }),
    attach: (sessionId, frameId) => watchers.forEach((fn) => fn(sessionId, frameId)),
  };
}

/** Channels made for iframes, each keeping its `receive` and what was asked of it. */
function channelMaker({ failing = [] } = {}) {
  const made = [];
  const make = (options) => {
    const child = { ...options, calls: [] };
    child.start = async () => {
      if (failing.includes(options.sessionId)) throw new Error('refused');
    };
    for (const name of ['drain', 'clear', 'stop']) child[name] = async () => child.calls.push(name);
    made.push(child);
    return child;
  };
  return { made, make };
}

describe('RemoteFrames', () => {
  it('arms the iframes already on the page and each one that attaches later', async () => {
    const frames = frameList([{ sessionId: 's1', frameId: 'f1' }]);
    const { made, make } = channelMaker();
    const remote = new RemoteFrames(parentChannel(), frames, make);
    await remote.armAll();
    frames.attach('s2', 'f2');
    assert.deepEqual(
      made.map((c) => c.sessionId),
      ['s1', 's2'],
    );
  });

  it('puts the iframe in front of each of its steps’ own frames', async () => {
    const parent = parentChannel(['iframe[id="pay"]']);
    const { made, make } = channelMaker();
    const remote = new RemoteFrames(parent, frameList([{ sessionId: 's1', frameId: 'f1' }]), make);
    await remote.armAll();
    await made[0].receive({ steps: [{ action: 'type', frames: ['iframe[name="card"]'] }] });
    assert.deepEqual(parent.received[0].steps[0].frames, ['iframe[id="pay"]', 'iframe[name="card"]']);
  });

  it('finds an iframe missing from the page’s frame tree by its owner element', async () => {
    const parent = parentChannel(null);
    const { made, make } = channelMaker();
    const remote = new RemoteFrames(parent, frameList([{ sessionId: 's1', frameId: 'f1' }]), make);
    await remote.armAll();
    await made[0].receive({ steps: [{ action: 'click' }] });
    assert.deepEqual(parent.received[0].steps[0].frames, ['iframe[name="widget"]']);
  });

  it('flags an iframe it could not arm instead of recording nothing there', async () => {
    const parent = parentChannel();
    const { make } = channelMaker({ failing: ['s1'] });
    const remote = new RemoteFrames(parent, frameList([{ sessionId: 's1', frameId: 'f1' }]), make);
    await remote.armAll();
    assert.equal(parent.received[0].steps[0].action, 'unsupported_frame');
    assert.equal(remote.children.size, 0);
  });

  it('stops every iframe’s channel and hears of no more', async () => {
    const frames = frameList([{ sessionId: 's1', frameId: 'f1' }]);
    const { made, make } = channelMaker();
    const remote = new RemoteFrames(parentChannel(), frames, make);
    await remote.armAll();
    await remote.stop();
    frames.attach('s2', 'f2');
    assert.deepEqual(made[0].calls, ['stop']);
    assert.equal(made.length, 1);
  });

  it('does nothing without a transport that reaches iframe sessions', async () => {
    const remote = new RemoteFrames(parentChannel(), null, () => assert.fail('no channel expected'));
    await remote.armAll();
    assert.equal(remote.owns('f1'), false);
  });

  it('flags a frame only when its session has not attached after a moment', () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    try {
      const frames = frameList([]);
      const remote = new RemoteFrames(parentChannel(), frames, channelMaker().make);
      const flagged = [];
      remote.flagUnlessArmed('late', () => flagged.push('late'));
      remote.flagUnlessArmed('never', () => flagged.push('never'));
      frames.list = () => [{ sessionId: 's1', frameId: 'late' }];
      mock.timers.tick(5000);
      assert.deepEqual(flagged, ['never']);
    } finally {
      mock.timers.reset();
    }
  });
});
