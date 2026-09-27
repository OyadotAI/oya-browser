/**
 * Unit tests for the HTML-to-Markdown conversion behind the docs' Markdown
 * twins, one rule per test, on small pieces of markup.
 */
import { describe, it, expect } from 'vitest';
import { docsMarkdown } from '@/app/docs/_docs/markdown';

/** The Markdown of `html` placed inside <main>. */
const md = (html: string) => docsMarkdown(`<html><body><nav>menu</nav><main>${html}</main></body></html>`).full;

describe('docsMarkdown', () => {
  it('writes headings at their level', () => {
    expect(md('<h2 id="a">Setup</h2><h3>Cursor</h3>')).toBe('## Setup\n\n### Cursor\n');
  });

  it('keeps code blocks exactly, and marks inline code', () => {
    expect(md('<pre><code><span>if (a &lt; b) {\n  go();\n}</span>\n</code></pre>')).toBe(
      '```\nif (a < b) {\n  go();\n}\n```\n',
    );
    expect(md('<p>Run <code>oya login</code> first.</p>')).toBe('Run `oya login` first.\n');
  });

  it('makes links absolute, anchors pointing into /docs', () => {
    expect(md('<p><a href="/dashboard">console</a> and <a href="#mfa">MFA</a></p>')).toBe(
      '[console](https://oyabrowser.com/dashboard) and [MFA](https://oyabrowser.com/docs#mfa)\n',
    );
  });

  it('writes lists, numbered and bulleted', () => {
    expect(md('<ul><li>one</li><li><strong>two</strong></li></ul><ol><li>a</li><li>b</li></ol>')).toBe(
      '- one\n- **two**\n\n1. a\n2. b\n',
    );
  });

  it('writes tables with a header row, escaping pipes', () => {
    const html =
      '<table><thead><tr><th>Key</th><th>Value</th></tr></thead><tbody><tr><td>a|b</td><td>c</td></tr></tbody></table>';
    expect(md(html)).toBe('| Key | Value |\n| --- | --- |\n| a\\|b | c |\n');
  });

  it('drops chrome marked data-md="skip", aria-hidden decoration and icons', () => {
    expect(
      md('<div data-md="skip">Copy</div><span aria-hidden="true">1</span><svg><path d="M0"></path></svg><p>kept</p>'),
    ).toBe('kept\n');
  });

  it('decodes character references', () => {
    expect(md('<p>Claude Desktop&#x27;s &quot;config&quot; &amp; more&#8230;</p>')).toBe(
      `Claude Desktop's "config" & more…\n`,
    );
  });

  it('splits out each section marked data-doc-page', () => {
    const docs = docsMarkdown('<main><p>intro</p><section data-doc-page="sdk"><h2>SDK</h2></section></main>');
    expect(docs.pages).toEqual({ sdk: '## SDK\n' });
    expect(docs.full).toBe('intro\n\n## SDK\n');
  });

  it('refuses a page with no <main>', () => {
    expect(() => docsMarkdown('<html><body>nothing</body></html>')).toThrow('no <main>');
  });
});
