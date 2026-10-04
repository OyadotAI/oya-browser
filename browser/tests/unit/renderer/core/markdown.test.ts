/**
 * Unit tests for the reply Markdown: tables, rules, lists, code and inline
 * styles render to the expected HTML, and everything else stays escaped text.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { markdownToHtml, escapeHtml } from '../../../../src/renderer/core/markdown.ts';

describe('the reply Markdown', () => {
  it('renders a pipe table with a header row', () => {
    const out = markdownToHtml('| Name | Fit |\n| :--- | :--- |\n| **Ann** | Good |');
    assert.match(out, /<thead><tr><th>Name<\/th><th>Fit<\/th><\/tr><\/thead>/);
    assert.match(out, /<tbody><tr><td><strong>Ann<\/strong><\/td><td>Good<\/td><\/tr><\/tbody>/);
  });

  it('renders a line of dashes as a rule, even without blank lines around it', () => {
    assert.equal(markdownToHtml('Intro\n---\nNext'), '<p>Intro</p><hr><p>Next</p>');
  });

  it('keeps pipes in a paragraph without a divider line as text', () => {
    assert.equal(markdownToHtml('a | b'), '<p>a | b</p>');
  });

  it('numbers a list that starts with a number, and bullets the rest', () => {
    assert.equal(markdownToHtml('1. one\n2. two'), '<ol><li>one</li><li>two</li></ol>');
    assert.equal(markdownToHtml('- a\n- *b*'), '<ul><li>a</li><li><em>b</em></li></ul>');
  });

  it('keeps a fenced code block verbatim, and escaped', () => {
    assert.equal(markdownToHtml('```js\n<b>**x**</b>\n```'), '<pre><code>&lt;b&gt;**x**&lt;/b&gt;</code></pre>');
  });

  it('escapes HTML in prose, so a reply cannot inject markup', () => {
    assert.equal(markdownToHtml('<img src=x onerror=alert(1)>'), '<p>&lt;img src=x onerror=alert(1)&gt;</p>');
  });

  it('escapes as the DOM serializes text: quotes stay, a no-break space is named', () => {
    assert.equal(escapeHtml('"a" & \'b\' <c>'), '"a" &amp; \'b\'&nbsp;&lt;c&gt;');
  });
});
