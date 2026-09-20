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

  it('reads an analysis with blocks in the format the browser wrote it in', () => {
    const data = {
      format: 'toon',
      facts: { url: 'https://a.test/' },
      blocks: [{ id: 1, region: 'main', kind: 'button', text: 'Go', target: '', state: '' }],
      elements: [{ id: 1, type: 'button', text: 'Go', visible: true }],
    };
    assert.ok(analysis(data).page.includes('blocks[1]{id,region,kind,text,target,state}:\n  1,main,button,Go,"",""'));
    const md = analysis({ ...data, format: 'markdown' }).page;
    assert.ok(md.includes('[#1 button "Go"]'), 'the element is in the page, tag and all');
    assert.ok(!md.includes('## Element Index'), 'and is not listed a second time after it');
    const jsonl = analysis({ ...data, format: 'jsonl' }).page.split('\n');
    assert.deepEqual(JSON.parse(jsonl[1]), { id: 1, region: 'main', kind: 'button', text: 'Go' });
  });

  it('keeps a cut TOON page valid: its row count matches the rows it kept', () => {
    const blocks = Array.from({ length: 4000 }, (_, i) => ({
      region: 'main',
      kind: 'text',
      text: `paragraph ${i} `.repeat(4),
    }));
    const page = analysis({ format: 'toon', facts: { url: 'https://a.test/' }, blocks, elements: [] }).page;
    const kept = Number(/blocks\[(\d+)\]/.exec(page)[1]);
    assert.ok(page.length <= 30_000 && kept < 4000);
    assert.equal(page.split('\n').filter((line) => line.startsWith('  "",main,text,')).length, kept);
    assert.match(page, /truncated: "showing \d+ of 4000 blocks/);
  });

  it('warns when the page was cut short', () => {
    assert.match(analysis({ markdown: '', elements: [], truncated: true }).page, /Page content was truncated/);
  });
});
