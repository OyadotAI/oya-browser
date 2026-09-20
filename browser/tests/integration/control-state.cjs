/**
 * Integration check of the desktop control state against a scripted server:
 * local drain, shared gate tokens, conflicting requests, stale updates, a
 * failed takeover, disconnects and servers without desktop control.
 */
const assert = require('node:assert/strict');
const { createControlState } = require('../../control-state.cjs');
(async () => {
  let server = { mode: 'agent', revision: 1 },
    failAcquire = false;
  const sent = [];
  const controller = createControlState({
    changed() {},
    send(message) {
      sent.push(message);
      queueMicrotask(() => {
        if (message.action === 'command-start') return controller.result({ id: message.id, token: 'command-slot' });
        if (message.action === 'command-end') return controller.result({ id: message.id });
        const mode = { request: 'paused', acquire: 'human', return: 'agent', renew: 'human' }[message.action];
        if (message.action === 'acquire' && failAcquire)
          return controller.result({ id: message.id, error: 'Command still running', state: server });
        server = { mode, mine: mode !== 'agent', expiresAt: Date.now() + 300000, revision: server.revision + 1 };
        controller.result({ id: message.id, state: server });
      });
      return true;
    },
  });
  try {
    assert(controller.snapshot().interactive);
    controller.localClient(1);
    assert(!controller.snapshot().interactive);
    const finish = await controller.beginLocalCommand();
    const taking = controller.change('acquire');
    await assert.rejects(controller.beginLocalCommand(), /paused/);
    await assert.rejects(controller.change('acquire'), /already/);
    finish();
    await taking;
    assert(controller.snapshot().interactive);
    await assert.rejects(controller.beginLocalCommand(), /paused/);
    await controller.change('return');
    controller.connect(server);
    const complete = await controller.beginLocalCommand();
    complete();
    await new Promise((resolve) => setImmediate(resolve));
    assert(sent.some((message) => message.action === 'command-end' && message.token === 'command-slot'));
    await controller.change('acquire');
    assert(controller.snapshot().interactive);
    controller.receive({ mode: 'agent', revision: 0 });
    assert.equal(controller.snapshot().mode, 'human', 'stale updates cannot grant automation');
    await controller.change('return');
    failAcquire = true;
    await assert.rejects(controller.change('acquire'), /still running/);
    assert.equal(controller.snapshot().mode, 'paused');
    assert(!controller.snapshot().interactive);
    await controller.change('return');
    failAcquire = false;
    await controller.change('acquire');
    controller.disconnect();
    await assert.rejects(controller.beginLocalCommand(), /paused/, 'disconnect must not resume local automation');
    await controller.change('acquire');
    assert(controller.snapshot().interactive);
    controller.connect(null);
    assert.equal(controller.snapshot().mode, 'unavailable');
    await assert.rejects(controller.change('acquire'), /does not support/);
    console.log(
      'Control state passed: local drain, shared gate tokens, conflicting requests, stale updates, failed takeover, disconnect and older servers.',
    );
  } finally {
    controller.dispose();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
