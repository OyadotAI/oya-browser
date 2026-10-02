/**
 * Unit tests for the live run in the Ask pane: the plan and the narrated steps
 * from the agent's events, only the run's own events, the summary it folds into,
 * and a reset when the chat is cleared mid-run.
 */
const { describe, it, beforeEach } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer } = require('../../support/renderer-harness.cjs');

describe('the live run', () => {
  let app;
  beforeEach(() => {
    app = loadRenderer();
    app.run('ChatRun.begin()');
  });
  /** Sends one agent event from the main process. */
  const agent = (runId, event) => app.bridge.emit('AgentEvent', { runId, event });
  const card = () => app.$('chat-messages').querySelector('.chat-run');

  it('draws the plan and one narrated line per action once its run has started', () => {
    agent('r1', { kind: 'start', task: 'find jordans' });
    agent('r1', {
      kind: 'plan',
      steps: [
        { step: 'Open amazon', done: true },
        { step: 'Search', done: false },
      ],
    });
    agent('r1', { kind: 'step', line: 'Opening amazon.com' });
    assert.deepEqual(
      [...card().querySelectorAll('.run-plan li')].map((li) => [li.textContent, li.classList.contains('done')]),
      [
        ['Open amazon', true],
        ['Search', false],
      ],
    );
    assert.match(card().querySelector('.run-step').textContent, /Opening amazon\.com/);
    assert.equal(app.$('panel-orb').dataset.state, 'thinking');
  });

  it('ignores events before its start, and any from another run', () => {
    agent('old', { kind: 'step', line: 'Clicking “Buy”' });
    agent('r1', { kind: 'start' });
    agent('old', { kind: 'step', line: 'Clicking “Buy”' });
    assert.equal(card().querySelectorAll('.run-step').length, 0);
  });

  it('folds into a summary that opens again, and shows how it went', () => {
    agent('r1', { kind: 'start' });
    agent('r1', { kind: 'step', line: 'Opening amazon.com' });
    app.run('ChatRun.finish(true)');
    const summary = card().querySelector('.run-summary');
    assert.match(summary.textContent, /^1 step · /);
    assert.ok(card().classList.contains('folded'));
    app.fire(summary, 'click');
    assert.ok(!card().classList.contains('folded'));
    assert.equal(app.$('panel-orb').dataset.state, 'done');
  });

  it('leaves no card for a run that had neither steps nor a plan', () => {
    app.run('ChatRun.finish(false)');
    assert.equal(card(), null);
    assert.equal(app.$('panel-orb').dataset.state, 'failed');
  });

  it('rests the orb and forgets the run when the chat is cleared mid-run', () => {
    app.run('Chat.clear()');
    assert.equal(app.$('panel-orb').dataset.state, 'idle');
    agent('r1', { kind: 'start' });
    agent('r1', { kind: 'step', line: 'Opening amazon.com' });
    assert.equal(app.$('chat-messages').querySelectorAll('.run-step').length, 0);
  });
});
