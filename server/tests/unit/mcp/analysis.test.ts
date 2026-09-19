/**
 * Unit tests for how analyze's result reads to an MCP client: the page markdown
 * followed by the same element index the agent reads.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { analysis } from '../../../src/mcp/analysis.ts';

describe('analysis', () => {
  it('follows the markdown with the element index: fields, then other visible elements', () => {
    const { page } = analysis({
      markdown: '# Title',
      elements: [
        { id: 1, type: 'link', text: 'Home', href: '/', visible: true },
        { id: 2, type: 'input', value: 'ada', visible: true },
      ],
    });
    assert.ok(page.startsWith('# Title\n\n## Element Index (2 total, 2 visible)\n\n'));
    assert.ok(page.includes('fields[1]{id,type,label,value,hint,state}:\n  2,text,"",ada,"",""\n'));
    assert.ok(page.includes('visible[1]{id,type,label,link}:\n  1,link,Home,/\n'));
    assert.ok(!page.includes('Off-screen'));
  });

  it('lists off-screen elements briefly, under their own heading', () => {
    const { page } = analysis({
      markdown: '',
      elements: [{ id: 3, type: 'button', text: 'More', href: '/x', disabled: true, visible: false }],
    });
    assert.ok(page.includes('(1 total, 0 visible)'));
    assert.ok(page.includes('Off-screen (scroll to reveal):\noffscreen[1]{id,type,label}:\n  3,button,More\n'));
    assert.ok(!page.includes('visible['));
  });

  it('caps a long page like the agent does, keeping the element index', () => {
    const { page } = analysis({
      markdown: 'y'.repeat(100_000),
      elements: [{ id: 1, type: 'link', text: 'Home', visible: true }],
    });
    assert.ok(page.length < 100_000);
    assert.ok(page.includes('⚠ Output truncated to fit context window.'));
    assert.ok(page.includes('visible[1]{id,type,label,link}:\n  1,link,Home,'));
  });

  it('shows the ARIA state of a visible element beside its label', () => {
    const { page } = analysis({
      markdown: '',
      elements: [{ id: 2, type: 'button', text: 'Menu', state: 'expanded', visible: true }],
    });
    assert.ok(page.includes('2,button,Menu (expanded),'));
  });

  it('shows a field’s ARIA state, and when it is covered, in its state column', () => {
    const { page } = analysis({
      markdown: '',
      elements: [{ id: 5, type: 'select', text: 'Country', state: 'expanded covered', visible: true }],
    });
    assert.ok(page.includes('5,select,Country,"","",expanded covered'));
  });

  it('warns when the page was cut short', () => {
    assert.match(analysis({ markdown: '', elements: [], truncated: true }).page, /Page content was truncated/);
  });
});
