/**
 * Unit tests for the live run in the Ask pane: the plan and the narrated steps
 * from the agent's events (only the run's own), the summary it folds into, the
 * orb, a reset when the chat is cleared mid-run, and the step line that
 * follows the browser's commands when the agent does not narrate.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { RunViewModel } from '../../../../../../src/renderer/features/ask/view-models/run-view-model.ts';
import { progressMeta } from '../../../../../../src/renderer/features/ask/model/steps.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';
import { ASK_TEXT } from '../../../../../../src/renderer/features/ask/model/constants.ts';
import { fakeBridge, type FakeBridge } from '../../../support/bridge.ts';

/** One activity entry for a command the server sent. */
const cmd = (action: string, params = {}) => ({
  dir: 'in',
  type: `cmd: ${action}`,
  data: JSON.stringify({ id: 'abc', action, params }),
});

describe('RunViewModel', () => {
  let fake: FakeBridge;
  let run: RunViewModel;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setInterval', 'setTimeout', 'Date'] });
    fake = fakeBridge();
    run = new RunViewModel(fake.bridge);
    run.begin();
  });
  afterEach(() => (run.dispose(), mock.timers.reset()));

  /** Sends one agent event from the main process. */
  const agent = (runId: string, event: object) => fake.emit('onAgentEvent', { runId, event });
  /** The step line as people read it. */
  const line = () => `${run.state.label || ASK_TEXT.thinking} ${progressMeta(run.state.count, run.state.seconds)}`;

  describe('the card', () => {
    it('draws the plan and one narrated line per action once its run has started', () => {
      agent('r1', { kind: 'start', task: 'find jordans' });
      agent('r1', { kind: 'plan', steps: [{ step: 'Open amazon', done: true }, { step: 'Search' }] });
      agent('r1', { kind: 'step', line: 'Opening amazon.com' });
      assert.deepEqual(run.state.card?.plan, [{ step: 'Open amazon', done: true }, { step: 'Search' }]);
      assert.deepEqual(run.state.card?.steps, [{ line: 'Opening amazon.com', time: '0s' }]);
      assert.equal(run.state.orb, 'thinking');
    });

    it('names a step by its tool when it has no line', () => {
      agent('r1', { kind: 'start' });
      agent('r1', { kind: 'step', tool: 'click' });
      assert.equal(run.state.card?.steps[0].line, 'click');
    });

    it('ignores events before its start, and any from another run', () => {
      agent('old', { kind: 'step', line: 'Clicking Buy' });
      agent('r1', { kind: 'start' });
      agent('old', { kind: 'step', line: 'Clicking Buy' });
      assert.equal(run.state.card?.steps.length, 0);
    });

    it('pulses the orb once for each step', () => {
      agent('r1', { kind: 'start' });
      agent('r1', { kind: 'step', line: 'a' });
      agent('r1', { kind: 'step', line: 'b' });
      assert.equal(run.state.pulse, 2);
    });

    it('folds into a summary of its steps and time, and the orb shows it went well, then rests', () => {
      agent('r1', { kind: 'start' });
      agent('r1', { kind: 'step', line: 'Opening amazon.com' });
      mock.timers.tick(4 * C.MS_PER_SECOND);
      const finished = run.finish(true);
      assert.equal(finished?.summary, '1 step · 4s');
      assert.equal(finished?.outcome, 'done');
      assert.equal(run.state.card, null);
      assert.equal(run.state.orb, 'done');
      mock.timers.tick(C.CHAT_ORB_REST_MS);
      assert.equal(run.state.orb, 'idle');
    });

    it('leaves no card for a run that had neither steps nor a plan', () => {
      assert.equal(run.finish(false), null);
      assert.equal(run.state.orb, 'failed');
    });

    it('keeps a plan-only run open, without a summary', () => {
      agent('r1', { kind: 'start' });
      agent('r1', { kind: 'plan', steps: [{ step: 'Search' }] });
      const finished = run.finish(true);
      assert.equal(finished?.summary, '');
      assert.equal(finished?.plan.length, 1);
    });

    it('finishes nothing when no run is in flight', () => {
      run.finish(true);
      assert.equal(run.finish(true), null);
    });

    it('rests the orb and forgets the run when the chat is cleared mid-run', () => {
      run.reset();
      assert.equal(run.state.orb, 'idle');
      agent('r1', { kind: 'start' });
      agent('r1', { kind: 'step', line: 'Opening amazon.com' });
      assert.equal(run.state.card, null);
    });
  });

  describe('the step line', () => {
    it('shows what the latest command is doing, and what it acts on', () => {
      fake.emit('onDevLog', cmd('navigate', { url: 'https://example.com' }));
      assert.match(line(), /Navigate, https:\/\/example\.com/);
    });

    it('names the page analysis in words rather than as an action', () => {
      fake.emit('onDevLog', cmd('analyze', { format: 'markdown' }));
      assert.match(line(), /Reading the page/);
    });

    it('names an action it has no words for by the action itself', () => {
      fake.emit('onDevLog', cmd('frobnicate'));
      assert.match(line(), /^frobnicate/);
    });

    it('counts the steps and shows only the latest', () => {
      fake.emit('onDevLog', cmd('navigate', { url: 'https://example.com' }));
      fake.emit('onDevLog', cmd('click', { selector: '[data-ac-id="4"]' }));
      assert.match(line(), /Click/);
      assert.doesNotMatch(line(), /example\.com/);
      assert.match(line(), /step 2/);
    });

    it('cuts a long detail to the room the line has', () => {
      fake.emit('onDevLog', cmd('type', { text: 'x'.repeat(200) }));
      assert.ok(line().length < 100, line());
      assert.match(line(), /…/);
    });

    it('still names the action when the entry’s JSON was truncated', () => {
      fake.emit('onDevLog', { dir: 'in', type: 'cmd: navigate', data: '{"id":"abc","par' });
      assert.match(line(), /Navigate/);
    });

    it('ignores results, outgoing messages and anything that is not a command', () => {
      fake.emit('onDevLog', { dir: 'out', type: 'result: ok', data: '{}' });
      fake.emit('onDevLog', { dir: 'in', type: 'hello', data: '{}' });
      assert.match(line(), /Thinking/);
      assert.doesNotMatch(line(), /step/);
    });

    it('ignores commands once the answer has come back', () => {
      run.finish(true);
      fake.emit('onDevLog', cmd('navigate', { url: 'https://example.com' }));
      assert.equal(run.state.label, '');
    });

    it('leaves the steps to the agent once it narrates the run', () => {
      agent('r1', { kind: 'start' });
      agent('r1', { kind: 'step', line: 'Opening amazon.com' });
      fake.emit('onDevLog', cmd('navigate', { url: 'https://amazon.com' }));
      assert.match(line(), /^Thinking… step 1/);
    });

    it('keeps the elapsed count moving between commands', () => {
      mock.timers.tick(3 * C.CHAT_PROGRESS_TICK_MS);
      assert.match(line(), /3s$/);
    });
  });
});
