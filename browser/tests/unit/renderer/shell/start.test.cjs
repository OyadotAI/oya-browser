/**
 * Unit tests for the Oya start page: it shows while the active tab is on it, and a
 * task typed there, or an example picked, goes to the agent through the Ask pane.
 */
const { describe, it, beforeEach } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer, settle } = require('../../support/renderer-harness.cjs');

describe('the start page', () => {
  let app;
  beforeEach(() => {
    app = loadRenderer({ answers: { sendChat: { text: 'DONE: found them', toolCalls: [] } } });
  });

  /** Reports one active tab, on the start page or on a site. */
  const activeTab = (home) =>
    app.bridge.emit('TabsUpdated', [{ id: 1, active: true, home, url: home ? '' : 'https://a.test/' }]);

  it('shows exactly while the active tab is on the start page', () => {
    activeTab(true);
    assert.equal(app.$('start-page').hidden, false);
    assert.ok(app.document.body.classList.contains('on-home'));
    activeTab(false);
    assert.equal(app.$('start-page').hidden, true);
    assert.ok(!app.document.body.classList.contains('on-home'));
  });

  it('hands a typed task to the agent, opening the panel on Ask', async () => {
    activeTab(true);
    app.$('start-input').value = 'Find Jordans on Amazon';
    app.fire(app.$('start-ask'), 'submit');
    await settle();
    assert.equal(app.bridge.called('toggleDevPanel').length, 1);
    const [history] = app.bridge.called('sendChat').at(-1);
    assert.equal(history.at(-1).content, 'Find Jordans on Amazon');
    assert.equal(app.$('start-input').value, '', 'the box is ready for the next task');
  });

  it('runs an example task as if it were typed', async () => {
    activeTab(true);
    const example = app.$('start-examples').querySelector('[data-task]');
    app.fire(example, 'click', { bubbles: true });
    await settle();
    const [history] = app.bridge.called('sendChat').at(-1);
    assert.equal(history.at(-1).content, example.dataset.task);
  });

  it('starts on Enter, keeps Shift+Enter for a new line, and ignores an empty box', async () => {
    activeTab(true);
    app.key(app.$('start-input'), 'Enter');
    app.$('start-input').value = 'a task';
    app.key(app.$('start-input'), 'Enter', { shiftKey: true });
    await settle();
    assert.equal(app.bridge.called('sendChat').length, 0);
    app.key(app.$('start-input'), 'Enter');
    await settle();
    assert.equal(app.bridge.called('sendChat').length, 1);
  });
});
