/**
 * Unit tests for scripts/recording.cjs: arming the recorder in every frame,
 * the handshake, delivering steps with their frame paths, and stopping.
 */
const { describe, it, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { RecordingChannel } = require('../../../scripts/recording.cjs');

/** A page over CDP: a frame tree, recorder worlds, and a recorder that answers the handshake. */
function fakePage({ answer = true, badFrames = [], drained = null } = {}) {
  const sent = [];
  const handlers = {};
  let nextContext = 10;
  const page = { sent, handlers, channel: null };
  const tree = {
    frame: { id: 'top' },
    childFrames: [{ frame: { id: 'f1' } }, ...badFrames.map((id) => ({ frame: { id } }))],
  };
  const answers = {
    'Page.getFrameTree': () => ({ frameTree: tree }),
    'Page.addScriptToEvaluateOnNewDocument': () => ({ identifier: 'script-1' }),
    'Page.createIsolatedWorld': (p) => {
      if (badFrames.includes(p.frameId)) throw new Error('cannot');
      return { executionContextId: nextContext++ };
    },
    'DOM.getFrameOwner': () => ({ backendNodeId: 7 }),
    'DOM.describeNode': () => ({ node: { localName: 'iframe', attributes: ['src', '/a', 'id', 'pay'] } }),
    'Runtime.evaluate': (p) => {
      if (p.expression.includes('\\"ready\\"') && answer)
        setImmediate(() => page.call({ ready: page.channel.binding }));
      if (p.expression.includes('Drain')) return { result: { value: drained } };
      return { result: {} };
    },
  };
  page.send = async (method, params = {}) => {
    sent.push({ method, params });
    return answers[method]?.(params) ?? {};
  };
  page.on = (event, fn) => {
    handlers[event] = fn;
    return () => delete handlers[event];
  };
  page.call = (data, executionContextId = 10) =>
    handlers['Runtime.bindingCalled']?.({
      name: page.channel.binding,
      payload: JSON.stringify(data),
      executionContextId,
    });
  page.methods = () => sent.map((s) => s.method);
  return page;
}

/** A channel on `page`, collecting what it receives. */
function channelOn(page, options = {}) {
  const received = [];
  page.channel = new RecordingChannel({
    send: page.send,
    on: page.on,
    worldName: 'rec',
    analyzer: '/* analyzer $& */',
    receive: (data) => received.push(data),
    ...options,
  });
  return { channel: page.channel, received };
}

describe('RecordingChannel', () => {
  afterEach(() => mock.timers.reset());

  it('arms the page and each frame in a private world, then completes the handshake', async () => {
    const page = fakePage();
    const { channel } = channelOn(page);
    await channel.start();
    const methods = page.methods();
    assert.deepEqual(methods.slice(0, 4), [
      'Page.getFrameTree',
      'Runtime.enable',
      'Runtime.addBinding',
      'Page.addScriptToEvaluateOnNewDocument',
    ]);
    assert.equal(methods.filter((m) => m === 'Page.createIsolatedWorld').length, 2);
    assert.match(channel.worldName, /^rec-r[0-9a-f]{24}$/);
    const script = page.sent.find((s) => s.method === 'Page.addScriptToEvaluateOnNewDocument').params;
    assert.equal(script.worldName, channel.worldName);
    assert.ok(script.source.includes('/* analyzer $& */'), 'the analyzer is inserted verbatim');
    assert.ok(script.source.includes(`window[${JSON.stringify(channel.binding)}]`));
  });

  it('flags a frame it cannot arm instead of failing', async () => {
    const page = fakePage({ badFrames: ['bad'] });
    const { channel, received } = channelOn(page);
    await channel.start();
    assert.equal(received[0].steps[0].action, 'unsupported_frame');
  });

  it('gives up and cleans up when the page never answers', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const page = fakePage({ answer: false });
    const { channel } = channelOn(page, { disableRuntimeOnStop: true });
    const started = channel.start();
    const handshake = () =>
      page.sent.some((s) => s.method === 'Runtime.evaluate' && s.params.expression.includes('\\"ready'));
    while (!handshake()) await new Promise((r) => setImmediate(r));
    await new Promise((r) => setImmediate(r));
    mock.timers.tick(3000);
    await assert.rejects(started, /did not connect/);
    assert.ok(page.methods().includes('Runtime.removeBinding'));
    assert.ok(page.methods().includes('Runtime.disable'));
    assert.deepEqual(page.handlers, {});
  });

  it('delivers steps tagged with the frame path they came from', async () => {
    const page = fakePage();
    const { channel, received } = channelOn(page);
    await channel.start();
    page.handlers['Runtime.executionContextCreated']({
      context: { id: 50, name: channel.worldName, auxData: { frameId: 'f1' } },
    });
    await channel.deliver({ steps: [{ action: 'click' }] }, 50);
    await channel.deliver({ steps: [{ action: 'type' }] }, 10);
    assert.deepEqual(received.at(-2).steps[0].frames, ['iframe[id="pay"]']);
    assert.deepEqual(received.at(-1).steps[0].frames, []);
  });

  it('keeps delivering after one delivery fails', async () => {
    const page = fakePage();
    let calls = 0;
    const receive = (data) => {
      calls++;
      if (data.steps[0].action === 'bad') throw new Error('boom');
    };
    const { channel } = channelOn(page, { receive });
    await channel.start();
    const logged = mock.method(console, 'error', () => {});
    await channel.deliver({ steps: [{ action: 'bad' }] }, 10);
    await channel.deliver({ steps: [{ action: 'click' }] }, 10);
    assert.equal(calls, 2);
    assert.equal(logged.mock.callCount(), 1);
    await assert.doesNotReject(channel.stop());
  });

  it('delivers a status message untouched and ignores other bindings and bad JSON', async () => {
    const page = fakePage();
    const { channel, received } = channelOn(page);
    await channel.start();
    page.call({ recording: true });
    page.handlers['Runtime.bindingCalled']({ name: 'other', payload: '{}' });
    page.handlers['Runtime.bindingCalled']({ name: channel.binding, payload: 'not json' });
    await channel.delivery;
    assert.deepEqual(received, [{ recording: true }]);
  });

  it('marks steps whose frame cannot be identified', async () => {
    const page = fakePage();
    const { channel, received } = channelOn(page);
    await channel.start();
    channel.contextFrames.set(77, 'gone');
    await channel.deliver({ steps: [{ action: 'click' }] }, 77);
    assert.match(received.at(-1).steps[0].captureIssue, /frame target could not be identified/);
  });

  it('drains every context, dropping ones navigation destroyed', async () => {
    const page = fakePage({ drained: { steps: [{ action: 'click' }] } });
    const { channel, received } = channelOn(page);
    await channel.start();
    const send = page.send;
    channel.send = async (method, params) => {
      if (method === 'Runtime.evaluate' && params.contextId === 11) throw new Error('Cannot find context with id');
      return send(method, params);
    };
    await channel.drain(true);
    assert.equal(received.length, 1);
    assert.ok(!channel.contexts.has(11));
  });

  it('rethrows evaluation errors that are not about a lost context', async () => {
    const page = fakePage();
    const { channel } = channelOn(page);
    await channel.start();
    channel.send = async () => ({ exceptionDetails: { exception: { description: 'boom' } } });
    await assert.rejects(channel.clear(), /boom/);
  });

  it('stops: removes the script and binding, forgets its contexts, keeps Runtime unless asked', async () => {
    const page = fakePage();
    const { channel } = channelOn(page);
    await channel.start();
    await channel.stop();
    const methods = page.methods();
    assert.ok(methods.includes('Page.removeScriptToEvaluateOnNewDocument'));
    assert.ok(methods.includes('Runtime.removeBinding'));
    assert.ok(!methods.includes('Runtime.disable'));
    assert.equal(channel.contexts.size, 0);
    assert.equal(channel.script, null);
    assert.deepEqual(page.handlers, {});
  });
});
