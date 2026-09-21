/**
 * Unit tests for scripts/page-render.cjs and its renderers: the factory picks a
 * renderer by format and refuses unknown ones, an analysis gains its page in
 * the format asked for, and each format renders and cuts the same page data.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { createRenderer, withPage, renderPage, FORMATS, DEFAULT_FORMAT } = require('../../../scripts/page-render.cjs');

/** A small analysis as the analyzer returns it: facts, block rows and elements. */
const analysis = () => ({
  facts: { url: 'https://shop.test/', title: 'Shop: home', scroll: '0% (0px of 900px)' },
  blocks: [
    { region: 'nav', kind: 'link', id: 1, text: 'Home', target: '/', state: '' },
    { region: 'main', kind: 'h1', text: 'Deals' },
    { region: 'main', kind: 'text', text: 'Prices, updated daily' },
    { region: 'main', kind: 'item', text: 'Lamp' },
    { region: 'main', kind: 'item', text: 'Chair' },
    { region: 'main', kind: 'header', text: 'Name | Price' },
    { region: 'main', kind: 'row', text: 'Lamp | $20' },
    {
      region: 'main',
      kind: 'input:email',
      id: 2,
      text: 'Email',
      target: 'a@b.test',
      state: 'required; hint: you@example.com',
    },
  ],
  elements: [
    { id: 1, type: 'link', text: 'Home', href: 'https://shop.test/', visible: true },
    { id: 2, type: 'input', inputType: 'email', text: 'Email', value: 'a@b.test', required: true, visible: true },
  ],
});

describe('createRenderer', () => {
  it('picks the renderer for a format, markdown when none is given', () => {
    assert.deepEqual(FORMATS, ['markdown', 'toon', 'jsonl']);
    assert.equal(DEFAULT_FORMAT, 'markdown');
    assert.equal(createRenderer().format, 'markdown');
    assert.equal(createRenderer('toon').format, 'toon');
  });

  it('refuses an unknown format, naming the ones there are', () => {
    assert.throws(() => createRenderer('xml'), /Unknown page format "xml". Use one of: markdown, toon, jsonl/);
  });

  it('gives every renderer the same interface', () => {
    for (const format of FORMATS) {
      const renderer = createRenderer(format);
      for (const method of ['render', 'fit', 'forAgent', 'controls'])
        assert.equal(typeof renderer[method], 'function', `${format}.${method}`);
      assert.equal(typeof renderer.guide, 'string');
    }
  });
});

describe('withPage', () => {
  it('adds the page in the format asked for, and markdown as before for markdown', () => {
    const md = withPage({ ok: true, data: analysis() }, 'markdown').data;
    assert.equal(md.format, 'markdown');
    assert.equal(md.markdown, md.page);
    const toon = withPage({ ok: true, data: analysis() }, 'toon').data;
    assert.equal(toon.format, 'toon');
    assert.equal(toon.markdown, undefined);
    assert.ok(toon.page.startsWith('page:\n'));
  });

  it('writes a kept analysis again in another format, the same page', () => {
    const data = withPage({ ok: true, data: analysis() }, 'markdown').data;
    assert.equal(renderPage(data, 'toon'), withPage({ ok: true, data: analysis() }, 'toon').data.page);
  });

  it('puts the facts in reading order, url first, however they arrived', () => {
    const data = analysis();
    data.facts = { title: 'Shop', elements: '1 total', url: 'https://shop.test/', extra: 'x' };
    assert.deepEqual(Object.keys(withPage({ ok: true, data }, 'toon').data.facts), [
      'url',
      'title',
      'elements',
      'extra',
    ]);
  });

  it('passes through an error or an older analysis without blocks', () => {
    const failed = { ok: false, error: 'Analyzer not loaded' };
    assert.equal(withPage(failed, 'toon'), failed);
    const older = { ok: true, data: { markdown: '# old' } };
    assert.equal(withPage(older, 'toon'), older);
  });
});

describe('toon renderer', () => {
  const toon = createRenderer('toon');

  it('writes the facts as an object and the blocks as one table', () => {
    const page = toon.render(analysis());
    assert.match(page, /^page:\n {2}url: "https:\/\/shop.test\/"\n {2}title: "Shop: home"/);
    assert.match(page, /\nblocks\[8\]\{id,region,kind,text,target,state\}:\n/);
    assert.ok(page.includes('\n  1,nav,link,Home,/,""'));
    assert.ok(page.includes('\n  "",main,text,"Prices, updated daily","",""'));
    // A colon is TOON syntax, so a value carrying one is quoted.
    assert.ok(page.includes('\n  2,main,"input:email",Email,a@b.test,"required; hint: you@example.com"'));
  });

  it('cuts at a row, with the row count and a truncated fact saying so', () => {
    const page = toon.render(analysis());
    const cut = toon.fit(page, page.length - 40);
    const header = cut.match(/blocks\[(\d+)\]/);
    const rows = cut.split('\n').filter((line) => /^ {2}[^ ]/.test(line) && !/^ {2}\w+: /.test(line));
    assert.equal(Number(header[1]), rows.length);
    assert.ok(Number(header[1]) < 8);
    assert.match(cut, /truncated: "showing \d+ of 8 blocks/);
  });
});

describe('markdown renderer', () => {
  const md = createRenderer('markdown');

  it('writes headings, lists, tables and elements as markdown, with regions as comments', () => {
    const page = md.render(analysis());
    assert.ok(page.startsWith('---\nurl: https://shop.test/\ntitle: Shop: home\n'));
    assert.ok(page.includes('<!-- nav -->\n\n[#1 link "Home" → /]'));
    assert.ok(page.includes('# Deals'));
    assert.ok(page.includes('- Lamp\n- Chair'));
    assert.ok(page.includes('| Name | Price |\n| --- | --- |\n| Lamp | $20 |'));
    assert.ok(page.includes('[#2 input:email "Email" = "a@b.test" (required; hint: you@example.com)]'));
  });

  it('makes a table without a header row a markdown table, its first row the header', () => {
    const page = md.render({
      facts: {},
      blocks: [
        { kind: 'row', text: 'Lamp | $20' },
        { kind: 'row', text: 'Chair | $40' },
      ],
    });
    assert.ok(page.endsWith('| Lamp | $20 |\n| --- | --- |\n| Chair | $40 |'));
  });

  it('gives the agent the page alone when it fits: its elements are already in it, tag by tag', () => {
    const text = md.forAgent(analysis(), 20_000);
    assert.ok(text.includes('[#2 input:email "Email" = "a@b.test"'), 'the field is in the page');
    assert.ok(!text.includes('## Element Index'), 'and not repeated in an index after it');
  });

  it('adds the element index to a page it had to cut, where the rest of the elements live', () => {
    const text = md.forAgent(analysis(), 200);
    assert.ok(text.includes('## Element Index'), 'the index comes back when the page is cut');
    assert.ok(text.includes('2,email,Email,a@b.test'), 'so an element past the cut is still reachable');
  });

  it('marks a page it had to cut short', () => {
    assert.ok(md.forAgent(analysis(), 200).includes('⚠ Output truncated to fit context window.'));
  });

  it('keeps the note about content the analyzer itself cut', () => {
    assert.match(md.forAgent({ ...analysis(), truncated: true }, 20_000), /Page content was truncated/);
  });
});

describe('jsonl renderer', () => {
  const jsonl = createRenderer('jsonl');

  it('writes the facts line, then one object per block without its empty fields', () => {
    const lines = jsonl
      .render(analysis())
      .split('\n')
      .map((line) => JSON.parse(line));
    assert.equal(lines.length, 9);
    assert.deepEqual(lines[0].page.title, 'Shop: home');
    assert.deepEqual(lines[1], { region: 'nav', kind: 'link', id: 1, text: 'Home', target: '/' });
    assert.deepEqual(lines[2], { region: 'main', kind: 'h1', text: 'Deals' });
  });

  it('cuts at a line, every line still JSON, the facts line saying how many blocks were kept', () => {
    const page = jsonl.render(analysis());
    const lines = jsonl
      .fit(page, page.length - 40)
      .split('\n')
      .map((line) => JSON.parse(line));
    assert.ok(lines.length < 9);
    assert.equal(
      lines[0].page.truncated,
      `showing ${lines.length - 1} of 8 blocks; scroll and analyze again for the rest`,
    );
  });
});

describe('controls', () => {
  it('gives every element and none of the page words, in every format', () => {
    for (const format of FORMATS) {
      const text = createRenderer(format).controls(analysis(), 20_000);
      assert.ok(text.includes('Email'), `${format} keeps the fields`);
      assert.ok(text.includes('Home'), `${format} keeps the links`);
      assert.ok(!text.includes('Prices, updated daily'), `${format} drops the page's words`);
      assert.ok(!text.includes('Deals'), `${format} drops the headings`);
    }
  });

  it('is smaller than the whole page', () => {
    const wordy = analysis();
    wordy.blocks.push({ region: 'main', kind: 'text', text: 'a long description of the shop '.repeat(40) });
    for (const format of FORMATS) {
      const renderer = createRenderer(format);
      const whole = renderer.forAgent(wordy, 20_000).length;
      assert.ok(renderer.controls(wordy, 20_000).length < whole, `${format} controls are smaller`);
    }
  });
});
