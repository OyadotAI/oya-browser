/**
 * Unit tests for the Inspect Source pane (renderer/panes/source.js): errors
 * and notices are shown, the page is named and marked stale, a newer format
 * switch always wins, and Refresh waits for its answer.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer, settle } = require('../../support/renderer-harness.cjs');

describe('the Source pane', () => {
  it('shows why the page could not be read, instead of saying there is nothing', async () => {
    const app = loadRenderer({ answers: { getPageSource: { html: '', markdown: '', error: 'Page is gone' } } });
    await settle();
    app.$('source-refresh').click();
    await settle();
    assert.match(app.$('source-markdown').textContent, /Could not read the page: Page is gone/);
  });

  it('shows the agent notice when the page cannot be read while it drives', async () => {
    const notice = 'The agent is using this page. Take control to read it.';
    const app = loadRenderer({
      answers: { getPageSource: { html: '<p>', markdown: '', notice, url: 'https://a.test/' } },
    });
    await settle();
    app.$('source-refresh').click();
    await settle();
    assert.equal(app.$('source-markdown').textContent, notice);
  });

  it('names the page it read, and says when the page changed', async () => {
    const app = loadRenderer({ answers: { getPageSource: { html: '<p>', markdown: '# A', url: 'https://a.test/' } } });
    await settle();
    app.$('source-refresh').click();
    await settle();
    assert.equal(app.$('source-url').textContent, 'Read from https://a.test/');
    app.bridge.emit('UrlChanged', 'https://b.test/');
    assert.match(app.$('source-url').textContent, /page changed/);
  });

  it('does not show an inspected element beside the HTML of another read', async () => {
    const app = loadRenderer({
      answers: { getPageSource: { html: '<p>old</p>', markdown: '# A', url: 'https://a.test/' } },
    });
    await settle();
    app.$('source-refresh').click();
    await settle();
    app.bridge.emit('InspectResult', { ok: true, data: { page: '# Button', blocks: [] } });
    await settle();
    assert.equal(app.$('source-markdown').textContent, '# Button');
    assert.doesNotMatch(app.$('source-html').textContent, /old/);
  });

  it('lets the newest format switch win over a slower earlier one', async () => {
    const pending = [];
    const app = loadRenderer({
      answers: {
        getPageSource: {
          html: '',
          markdown: '# A',
          analysis: { format: 'markdown', blocks: [] },
          url: 'https://a.test/',
        },
        renderPage: (_analysis, format) => new Promise((resolve) => pending.push(() => resolve('as ' + format))),
      },
    });
    await settle();
    app.$('source-refresh').click();
    await settle();
    for (const format of ['toon', 'jsonl']) {
      for (const radio of app.document.querySelectorAll('input[name="source-format"]'))
        radio.checked = radio.value === format;
      app.run('SourcePane.show()');
    }
    pending[1]();
    await settle();
    pending[0]();
    await settle();
    assert.equal(app.$('source-markdown').textContent, 'as jsonl');
  });

  it('keeps Refresh off while it reads, and its label the same afterwards', async () => {
    let release;
    const app = loadRenderer({ answers: { getPageSource: () => new Promise((resolve) => (release = resolve)) } });
    await settle();
    app.$('source-refresh').click();
    assert.equal(app.$('source-refresh').disabled, true);
    release({ html: '', markdown: '' });
    await settle();
    assert.equal(app.$('source-refresh').disabled, false);
    assert.equal(app.$('source-refresh').textContent, 'Refresh');
  });
});
