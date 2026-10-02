/**
 * Unit tests for the chrome's sign of an agent at work: it follows the commands
 * actually arriving, not the control mode, and settles after the last one.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer } = require('../../support/renderer-harness.cjs');

describe('agent activity in the chrome', () => {
  let app;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setTimeout'] });
    app = loadRenderer();
  });
  afterEach(() => mock.timers.reset());

  /** One activity entry, as the main process mirrors it. */
  const log = (dir, type) => app.bridge.emit('DevLog', { ts: 0, dir, type, data: '{}' });

  it('shows the agent at work while commands arrive, then settles', () => {
    log('in', 'cmd: click');
    assert.equal(app.document.documentElement.dataset.agentActive, 'true');
    assert.equal(app.$('brand-orb').dataset.state, 'thinking');
    mock.timers.tick(app.run('RendererConstants.AGENT_ACTIVE_MS'));
    assert.equal(app.document.documentElement.dataset.agentActive, 'false');
    assert.equal(app.$('brand-orb').dataset.state, 'idle');
  });

  it('ignores replies and anything that is not a command', () => {
    log('out', 'result');
    log('in', 'auth_ok');
    assert.notEqual(app.document.documentElement.dataset.agentActive, 'true');
  });
});
