/**
 * Unit tests for the Ask pane's live step line: what it shows for the commands
 * the server sends while a question is in flight, and what it ignores.
 */
const { describe, it, beforeEach, afterEach } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer } = require('../../support/renderer-harness.cjs');

/** One activity entry for a command the server sent. */
const cmd = (action, params = {}) => ({
  ts: Date.now(),
  dir: 'in',
  type: `cmd: ${action}`,
  data: JSON.stringify({ id: 'abc', action, params }),
});

describe('the Ask pane step line', () => {
  let app;

  /** A loaded renderer with the thinking line up and progress running. */
  beforeEach(() => {
    app = loadRenderer();
    app.run('Chat.showThinking(); ChatProgress.start();');
  });

  /** Stops the ticker the renderer started, so no timer outlives the test. */
  afterEach(() => app.run('ChatProgress.stop();'));

  /** The step line's text, with its whitespace collapsed. */
  const line = () => app.$('chat-messages').querySelector('.chat-thinking').textContent.replace(/\s+/g, ' ').trim();

  it('shows what the latest command is doing, and what it acts on', () => {
    app.bridge.emit('DevLog', cmd('navigate', { url: 'https://example.com' }));
    assert.match(line(), /Navigate — https:\/\/example\.com/);
  });

  it('names the page analysis in words rather than as an action', () => {
    app.bridge.emit('DevLog', cmd('analyze', { format: 'markdown' }));
    assert.match(line(), /Reading the page/);
  });

  it('counts the steps and shows only the latest', () => {
    app.bridge.emit('DevLog', cmd('navigate', { url: 'https://example.com' }));
    app.bridge.emit('DevLog', cmd('click', { selector: '[data-ac-id="4"]' }));
    assert.match(line(), /Click/);
    assert.doesNotMatch(line(), /example\.com/);
    assert.match(line(), /step 2/);
  });

  it('cuts a long detail to the room the line has', () => {
    app.bridge.emit('DevLog', cmd('type', { text: 'x'.repeat(200) }));
    assert.ok(line().length < 100, line());
    assert.match(line(), /…/);
  });

  it('still names the action when the entry’s JSON was truncated', () => {
    app.bridge.emit('DevLog', { ts: Date.now(), dir: 'in', type: 'cmd: navigate', data: '{"id":"abc","par' });
    assert.match(line(), /Navigate/);
  });

  it('ignores results, outgoing messages and anything that is not a command', () => {
    app.bridge.emit('DevLog', { ts: Date.now(), dir: 'out', type: 'result: ok', data: '{}' });
    app.bridge.emit('DevLog', { ts: Date.now(), dir: 'in', type: 'hello', data: '{}' });
    assert.match(line(), /Thinking/);
    assert.doesNotMatch(line(), /step/);
  });

  it('ignores commands once the answer has come back', () => {
    app.run('ChatProgress.stop();');
    app.bridge.emit('DevLog', cmd('navigate', { url: 'https://example.com' }));
    assert.match(line(), /Thinking/);
  });
});
