/**
 * Unit tests for a chat run's live events: what the agent is doing, pushed to the
 * browser that asked, so its panel can show a plan and a step timeline.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { LiveRun, stepLine } from '../../../../src/modules/agent/run-events.ts';
import { connectBrowser, disconnectBrowser } from '../../support/fakes.ts';

const B = 'b-live';

describe('run events', () => {
  afterEach(() => disconnectBrowser(B));

  /** The events the browser's socket received, in order. */
  const eventsOn = (ws) => ws.ofType('agent-event').map((m) => m.event);

  it('narrates each step in a few words, naming the element when it has one', () => {
    assert.equal(stepLine('navigate', { url: 'https://www.amazon.com/s?k=x' }), 'Opening amazon.com');
    assert.equal(stepLine('click', { element_id: 3 }, { text: 'Start  now' }), 'Clicking “Start now”');
    assert.equal(stepLine('type', { element_id: 4, text: 'x' }, { placeholder: 'Email' }), 'Typing into “Email”');
    assert.equal(stepLine('click', { element_id: 9 }), 'Clicking');
    assert.equal(stepLine('analyze_page', {}), 'Reading the page');
    assert.equal(stepLine('press_key', { key: 'Enter' }), 'Pressing Enter');
    assert.equal(stepLine('mouse_move', {}), '');
  });

  it('sends a run’s start, plan, steps and end to the browser, in order, under one run id', () => {
    const ws = connectBrowser(B);
    const run = new LiveRun(B);
    run.start('find jordans');
    run.toolCall({ name: 'update_plan', args: { steps: [{ step: 'Open Amazon', done: false }] } });
    run.toolCall({ name: 'navigate', args: { url: 'https://amazon.com' } });
    run.done({ failed: false });
    assert.deepEqual(eventsOn(ws), [
      { kind: 'start', task: 'find jordans' },
      { kind: 'plan', steps: [{ step: 'Open Amazon', done: false }] },
      { kind: 'step', tool: 'navigate', line: 'Opening amazon.com' },
      { kind: 'done', ok: true },
    ]);
    assert.equal(new Set(ws.ofType('agent-event').map((m) => m.runId)).size, 1);
  });

  it('never sends what was typed or chosen, or a secret', () => {
    const ws = connectBrowser(B);
    const run = new LiveRun(B, { password: 'hunter2' });
    run.toolCall({ name: 'type', args: { element_id: 1, text: 'my typed words' } });
    run.toolCall({ name: 'select_option', args: { element_id: 1, value: '1990-01-01' } });
    run.toolCall({ name: 'update_plan', args: { steps: [{ step: 'Sign in with hunter2', done: true }] } });
    const sent = JSON.stringify(ws.sent);
    assert.doesNotMatch(sent, /my typed words|1990-01-01|hunter2/);
  });

  it('says a run that threw failed, and a run that reported failure ended not ok', () => {
    const ws = connectBrowser(B);
    new LiveRun(B).failed(new Error('provider down'));
    new LiveRun(B).done({ failed: true });
    assert.deepEqual(eventsOn(ws), [
      { kind: 'failed', error: 'provider down' },
      { kind: 'done', ok: false },
    ]);
  });

  it('never throws when the browser cannot be told, so the run goes on', () => {
    const ws = connectBrowser(B);
    ws.failWith = new Error('socket closed');
    assert.doesNotThrow(() => new LiveRun(B).start('go'));
    assert.doesNotThrow(() => new LiveRun('not-connected').start('go'));
  });
});
