/**
 * Unit tests for src/page/render.ts and its renderers: the factory picks a
 * renderer by format and refuses unknown ones, an analysis gains its page in
 * the format asked for, and each format renders and cuts the same page data.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  createRenderer,
  withPage,
  renderPage,
  elementIndex,
  FORMATS,
  DEFAULT_FORMAT,
} from '../../../src/page/render.ts';
import { PAGE_RENDER } from '../../../src/page/constants.ts';
import type { Block } from '../../../src/page/render.ts';

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
  it('picks the renderer for a format, TOON when none is given', () => {
    assert.deepEqual(FORMATS, ['markdown', 'toon', 'jsonl']);
    assert.equal(DEFAULT_FORMAT, 'toon');
    assert.equal(createRenderer().format, 'toon');
    assert.equal(createRenderer('toon').format, 'toon');
  });

  it('refuses an unknown format, naming the ones there are', () => {
    assert.throws(() => createRenderer('xml'), /Unknown page format "xml". Use one of: markdown, toon, jsonl/);
  });

  it('gives every renderer the same interface', () => {
    for (const format of FORMATS) {
      const renderer = createRenderer(format);
      for (const method of ['render', 'cut', 'forAgent', 'controls'])
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

/**
 * Acceptance tests for the scrolled-page cut. A long list scrolled down: the analyzer returns the
 * whole document from the top, marks the on-screen elements, and the page is cut to the agent's
 * budget. The agent must see what is on screen.
 */
describe('cut keeps the on-screen part of a long page', () => {
  /** The agent's budget: server/src/modules/agent/constants.ts MAX_ANALYSIS_CHARS. */
  const BUDGET = 20_000;
  const ROWS = 600;
  const title = (i: number) => `Result ${i} - some product title text`;
  const price = (i: number) => `price $${i}.99 rating 4.${i % 10}`;

  /** `ROWS` links, each followed by its price text, with results `from`..`to` on screen. */
  const longPage = (from: number, to: number, rows = ROWS) => {
    const on = (i: number) => i >= from && i <= to;
    const blocks = [{ region: 'main', kind: 'h1', text: 'Results' }];
    const elements = [];
    for (let i = 0; i < rows; i++) {
      const state = on(i) ? '' : 'off-screen';
      blocks.push({
        id: i + 1,
        region: 'main',
        kind: 'link',
        text: title(i),
        target: `/item/${i}`,
        state,
        screen: on(i),
      } as any);
      blocks.push({ region: 'main', kind: 'item', text: price(i) });
      elements.push({ id: i + 1, type: 'link', text: title(i), href: `/item/${i}`, visible: on(i) });
    }
    const facts = { url: 'https://shop.test/list', title: 'List', scroll: '92% (8180px of 10896px)' };
    return { facts, blocks, elements };
  };
  const range = (from: number, to: number) => Array.from({ length: to - from + 1 }, (_, k) => from + k);
  const missing = (text: string, items: string[]) => items.filter((item) => !text.includes(item));

  it('R1 TOON shows every on-screen element after the cut', () => {
    const text = createRenderer('toon').forAgent(longPage(450, 520), BUDGET);
    assert.deepEqual(missing(text, range(450, 520).map(title)), []);
  });

  it('R2 TOON shows the content rows of the on-screen part, not only its elements', () => {
    const text = createRenderer('toon').forAgent(longPage(450, 520), BUDGET);
    assert.deepEqual(missing(text, range(450, 520).map(price)), []);
  });

  it('R3 TOON cut stays valid: within budget, facts whole, row count true, truncated said', () => {
    const text = createRenderer('toon').forAgent(longPage(450, 520), BUDGET);
    assert.ok(text.length <= BUDGET);
    assert.match(text, /^page:\n {2}url: "https:\/\/shop.test\/list"\n {2}title: List\n {2}scroll: /);
    assert.ok(text.includes(title(450)), 'the cut is placed on the on-screen part');
    const header = text.match(/\nblocks\[(\d+)\]\{id,region,kind,text,target,state\}:\n/);
    const rows = text.split('\n').filter((line) => /^ {2}[^ ]/.test(line) && !/^ {2}\w+: /.test(line));
    assert.equal(Number(header?.[1]), rows.length);
    assert.match(text, /truncated: "showing \d+ of 1201 blocks/);
  });

  it('R4 every format: a page that fits is not changed', () => {
    for (const format of FORMATS) {
      const renderer = createRenderer(format);
      const small = analysis();
      assert.equal(renderer.forAgent(small, BUDGET), renderer.render(small), format);
    }
  });

  it('R5 every format: a long page at the top still shows its top rows', () => {
    for (const format of FORMATS) {
      const text = createRenderer(format).forAgent(longPage(0, 40), BUDGET);
      assert.deepEqual(missing(text, ['Results', ...range(0, 40).map(title), ...range(0, 40).map(price)]), [], format);
    }
  });

  it('R6 JSONL shows every on-screen element and its content rows after the cut', () => {
    const text = createRenderer('jsonl').forAgent(longPage(450, 520), BUDGET);
    assert.deepEqual(missing(text, [...range(450, 520).map(title), ...range(450, 520).map(price)]), []);
    for (const line of text.split('\n')) JSON.parse(line);
  });

  it('R7 (default) every format: controls() keeps the on-screen elements after the cut', () => {
    for (const format of FORMATS) {
      const text = createRenderer(format).controls(longPage(450, 520), BUDGET);
      assert.ok(text.length <= BUDGET, format);
      assert.deepEqual(missing(text, range(450, 520).map(title)), [], format);
    }
  });

  it('R8 (default) markdown shows the content rows of the on-screen part, not only the index', () => {
    const text = createRenderer('markdown').forAgent(longPage(450, 520), BUDGET);
    assert.deepEqual(missing(text, range(450, 520).map(price)), []);
  });

  it('R9 (default) every format: with nothing marked on screen the cut keeps the top, as before', () => {
    for (const format of FORMATS) {
      const text = createRenderer(format).forAgent(longPage(-1, -1), BUDGET);
      assert.deepEqual(missing(text, ['Results', title(0), price(0)]), [], format);
      // The note itself says the cut is from the top, never around the screen.
      assert.match(text, /showing \d+ of \d+ blocks from the top/, format);
      assert.doesNotMatch(text, /around what is on screen/, format);
    }
  });

  it('a fixed nav bar on screen does not pull the cut back to the top', () => {
    const page = longPage(450, 520);
    page.blocks.unshift({
      id: 9000,
      region: 'nav',
      kind: 'link',
      text: 'Shop home',
      target: '/',
      state: '',
      screen: true,
    } as any);
    for (const format of ['toon', 'jsonl', 'markdown']) {
      const text = createRenderer(format).forAgent(page, BUDGET);
      assert.deepEqual(missing(text, [...range(450, 520).map(title), ...range(450, 520).map(price)]), [], format);
      assert.ok(text.includes('Shop home'), `${format} keeps the nav link`);
      assert.ok(text.length <= BUDGET, format);
    }
  });

  it('an off-screen form field beside the screen is kept, by its distance like the rows around it', () => {
    // The field tier is gone: a field next to the on-screen part is kept because it is near, not because it is a field.
    const page = longPage(450, 520);
    page.blocks.splice(901, 0, {
      id: 9001,
      region: 'form',
      kind: 'input:email',
      text: 'Newsletter email',
      state: 'off-screen',
    } as any);
    for (const format of ['toon', 'jsonl']) {
      assert.ok(createRenderer(format).forAgent(page, BUDGET).includes('Newsletter email'), format);
    }
  });

  it('the stored page keeps the on-screen part too', () => {
    const data = longPage(1800, 1850, 2000);
    for (const format of FORMATS) {
      const page = renderPage(data, format);
      assert.ok(page.includes(title(1800)) && page.includes(title(1850)), format);
      assert.ok(page.length <= PAGE_RENDER.MAX_PAGE_CHARS, format);
    }
  });

  it('markdown forAgent stays within budget on a long page', () => {
    assert.ok(createRenderer('markdown').forAgent(longPage(450, 520), BUDGET).length <= BUDGET);
  });

  it('an oversize block beside the screen does not push out the on-screen elements', () => {
    const blocks: Block[] = [{ region: 'main', kind: 'h1', text: 'Docs' }];
    for (let i = 0; i < ROWS; i++) {
      const on = i >= 450 && i <= 470;
      blocks.push({
        id: i + 1,
        region: 'main',
        kind: 'link',
        text: `Link ${i}`,
        target: `/l/${i}`,
        state: on ? '' : 'off-screen',
        screen: on,
      });
      blocks.push(
        i === 449
          ? { region: 'main', kind: 'code', text: 'x'.repeat(30000) }
          : { region: 'main', kind: 'text', text: `para ${i}` },
      );
    }
    const page = { facts: { url: 'https://d.test/', title: 'Docs' }, blocks, elements: [] };
    for (const format of FORMATS) {
      const text = createRenderer(format).forAgent(page, BUDGET);
      assert.deepEqual(
        missing(
          text,
          range(450, 470).map((i) => `Link ${i}`),
        ),
        [],
        format,
      );
      assert.ok(text.length <= BUDGET, format);
    }
  });

  it('a sticky nav on screen does not spend the budget on the top of the page', () => {
    const paragraph = (text: string): Block => ({ region: 'main', kind: 'text', text: text.repeat(4) });
    const blocks: Block[] = [{ id: 1, region: 'nav', kind: 'link', text: 'Home', target: '/', state: '' }];
    for (let i = 0; i < 500; i++) blocks.push(paragraph(`paragraph ${i} `));
    for (let i = 0; i < 5; i++)
      blocks.push({
        id: 100 + i,
        region: 'main',
        kind: 'link',
        text: `OnScreen ${i}`,
        target: '/x',
        state: '',
        screen: true,
      });
    for (let i = 0; i < 500; i++) blocks.push(paragraph(`tail ${i} `));
    const page = { facts: { url: 'https://d.test/', title: 'Docs' }, blocks, elements: [] };
    for (const format of FORMATS) {
      const text = createRenderer(format).forAgent(page, 6000);
      assert.deepEqual(
        missing(
          text,
          range(0, 4).map((i) => `OnScreen ${i}`),
        ),
        [],
        format,
      );
      assert.ok(text.length <= 6000, format);
    }
  });

  it("markdown's note counts the blocks it really shows", () => {
    const blocks: Block[] = Array.from({ length: 300 }, (_, i) => ({
      id: i + 1,
      region: `r${i}`,
      kind: 'link',
      state: '',
    }));
    const text = createRenderer('markdown').forAgent({ facts: { url: 'https://d.test/' }, blocks, elements: [] }, 5000);
    const shown = (text.match(/\[#/g) || []).length;
    assert.equal(Number(/showing (\d+) of 300 blocks/.exec(text)?.[1]), shown);
    assert.ok(shown > 0 && shown < 300, 'some blocks are shown and some are left out');
    assert.ok(text.length <= 5000);
  });

  it('markdown marks a gap in the kept blocks with a line, and a run never joins across it', () => {
    const blocks: Block[] = [
      { region: 'main', kind: 'item', text: 'Lamp' },
      { region: 'main', kind: 'item', text: 'Chair' },
      { region: 'main', kind: 'text', text: 'f'.repeat(900) },
      { id: 1, region: 'main', kind: 'link', text: 'Middle', target: '/m', state: '', screen: true },
      { region: 'main', kind: 'item', text: 'Sofa' },
      { region: 'main', kind: 'item', text: 'Desk' },
    ];
    const text = createRenderer('markdown').cut({ facts: { url: 'https://d.test/' }, blocks }, 600);
    // The 900-char text is clipped to its share of the room and shown in part; no gap is left, so it is the run check that matters.
    assert.ok(text.includes('fff'), 'the big block is shown in part');
    assert.ok(!text.includes('- Chair\n- Sofa'), 'the two lists are not one run');
    assert.ok(text.includes('- Sofa\n- Desk'), 'the rows after the gap stay one run');
  });

  it('text beside an on-screen element is kept before far off-screen form fields', () => {
    const blocks: Block[] = [];
    for (let i = 0; i < 200; i++)
      blocks.push({ id: i + 1, region: 'form', kind: 'input:text', text: `Field ${i}`, state: 'off-screen' });
    blocks.push({ region: 'main', kind: 'h2', text: 'VISIBLE HEADING' });
    blocks.push({ region: 'main', kind: 'text', text: 'VISIBLE PARAGRAPH before' });
    blocks.push({
      id: 900,
      region: 'main',
      kind: 'link',
      text: 'OnScreen link',
      target: '/x',
      state: '',
      screen: true,
    });
    blocks.push({ region: 'main', kind: 'text', text: 'VISIBLE PARAGRAPH after' });
    const page = { facts: { url: 'https://f.test/', title: 'Form' }, blocks, elements: [] };
    for (const format of FORMATS) {
      const text = createRenderer(format).forAgent(page, 3000);
      for (const want of ['OnScreen link', 'VISIBLE PARAGRAPH before', 'VISIBLE PARAGRAPH after', 'VISIBLE HEADING']) {
        assert.ok(text.includes(want), `${format} keeps ${want}`);
      }
      assert.ok(text.length <= 3000, format);
    }
  });

  it('markdown fills the budget it is given', () => {
    const text = createRenderer('markdown').forAgent(longPage(450, 520), BUDGET);
    assert.ok(text.length >= BUDGET - 300 && text.length <= BUDGET, `length ${text.length}`);
  });

  it('markdown marks blocks left out below the last shown one', () => {
    const blocks: Block[] = [{ id: 1, region: 'main', kind: 'link', text: 'Top', target: '/t', state: '' }];
    for (let i = 0; i < 20; i++) blocks.push({ region: 'main', kind: 'text', text: 'a'.repeat(200) });
    const text = createRenderer('markdown').cut({ facts: { url: 'https://d.test/' }, blocks, elements: [] }, 1000);
    assert.ok(text.endsWith('…'), 'the page part ends with the gap line');
    assert.ok(text.length <= 1000);
  });

  it('markdown skips a block too big to fit and keeps the blocks after it', () => {
    // An on-screen link anchors the cut; the 2950-char text beside it is too big for the room, so it is skipped and the fields after it stay.
    const blocks: Block[] = [
      { id: 1, region: 'main', kind: 'link', text: 'On screen link', target: '/o', state: '', screen: true },
      { region: 'main', kind: 'text', text: 'p'.repeat(2950) },
    ];
    for (let i = 0; i < 5; i++) {
      blocks.push({ id: 10 + i, region: 'form', kind: 'input:text', text: `Field ${i} name`, state: 'off-screen' });
    }
    const text = createRenderer('markdown').forAgent({ facts: { url: 'https://d.test/' }, blocks, elements: [] }, 3000);
    assert.ok(text.includes('On screen link'));
    for (let i = 0; i < 5; i++) assert.ok(text.includes(`Field ${i} name`), `field ${i}`);
    assert.ok(text.length <= 3000);
  });

  it("the note keeps the analyzer's own total after a second cut", () => {
    for (const format of FORMATS) {
      const text = createRenderer(format).forAgent({ ...longPage(450, 520), totalBlocks: 6000 }, BUDGET);
      assert.ok(text.includes('of 6000 blocks'), format);
    }
  });

  it('an unanchored markdown cut keeps one unbroken run from the top', () => {
    const blocks: Block[] = [];
    for (let i = 0; i < 60; i++) {
      blocks.push({ region: `r${i}`, kind: 'text', text: i % 3 === 2 ? 's' : 'long text block number '.repeat(3) + i });
    }
    const text = createRenderer('markdown').cut({ facts: { url: 'https://d.test/' }, blocks, elements: [] }, 674);
    const body = text.slice(text.indexOf('\n\n', text.indexOf('---', 4)) + 2);
    const runs = body.split('…');
    assert.ok(runs.length === 1 || runs[1].trim() === '', 'at most one gap, and only at the end');
    assert.ok(runs[0].includes('long text block number'), 'the top of the page is shown');
    assert.ok(/from the top/.test(text), 'the note says the cut is from the top');
    assert.ok(text.length <= 674);

    // The stop block is too big only because of its region comment (a long region name the ranking does not count).
    const longRegion = 'R'.repeat(120);
    const stop: Block[] = [
      { region: 'r0', kind: 'text', text: 'a'.repeat(30) },
      { region: longRegion, kind: 'text', text: 'b'.repeat(20) },
      { region: longRegion, kind: 'text', text: 'ccc' },
      { region: 'r2', kind: 'text', text: 'd'.repeat(3000) },
    ];
    const stopText = createRenderer('markdown').cut(
      { facts: { url: 'https://d.test/' }, blocks: stop, elements: [] },
      300,
    );
    assert.ok(!stopText.includes('ccc'), 'nothing after the block that stops the prefix is shown');
  });

  it('a page cannot stall the markdown cut', () => {
    const blocks: Block[] = Array.from({ length: 5000 }, (_, i) => ({ region: `r${i}`, kind: 'text', text: 'x' }));
    blocks[2500] = {
      id: 1,
      region: 'main',
      kind: 'link',
      text: 'On screen link',
      target: '/o',
      state: '',
      screen: true,
    };
    const started = performance.now();
    const text = createRenderer('markdown').forAgent(
      { facts: { url: 'https://d.test/' }, blocks, elements: [] },
      20000,
    );
    const took = performance.now() - started;
    assert.ok(took < 2000, `took ${took} ms`);
    assert.ok(text.includes('On screen link'));
    assert.ok(text.length <= 20000);
  });

  it('an unanchored markdown page whose first block is too big still shows its top', () => {
    const blocks: Block[] = [
      { kind: 'code', text: 'x'.repeat(5000) },
      { kind: 'text', text: 'after' },
    ];
    const text = createRenderer('markdown').forAgent({ facts: { url: 'https://d.test/' }, blocks, elements: [] }, 2000);
    assert.ok(text.includes('xxxx'));
    assert.ok(text.length <= 2000);
  });

  // Adapted: the on-screen link's region name alone is bigger than the room, so nothing around the screen fits and the
  // page is cut from the top, marked truncated. The note must not claim the screen.
  it('an anchored markdown cut whose on-screen block cannot fit is marked truncated and does not claim the screen', () => {
    const long = 'R'.repeat(500);
    const blocks: Block[] = [];
    for (let i = 0; i < 40; i++) blocks.push({ region: 'main', kind: 'text', text: `TOPTEXT ${i}` });
    for (let i = 0; i < 12; i++) blocks.push({ region: long, kind: 'text', text: `long ${i}` });
    blocks.push({ id: 1, region: long, kind: 'link', text: 'On screen link', target: '/o', state: '', screen: true });
    for (let i = 0; i < 12; i++) blocks.push({ region: long, kind: 'text', text: `tail ${i}` });
    const text = createRenderer('markdown').cut({ facts: { url: 'https://d.test/' }, blocks, elements: [] }, 400);
    assert.ok(text.includes('⚠ Output truncated to fit context window.'), 'the page says it was cut');
    assert.ok(!text.includes('around what is on screen'), 'the page does not claim the part on screen');
    assert.ok(text.length <= 400);
  });
});

describe('page gaps, notes and guides', () => {
  it('M1 markdown writes a gap where the analyzer cap left rows out, and none after the last row', () => {
    const blocks: Block[] = [
      { kind: 'header', text: 'A', index: 0 },
      { kind: 'row', text: '1', index: 1 },
      { kind: 'header', text: 'B', index: 7 },
      { kind: 'row', text: '2', index: 8 },
    ];
    const page = createRenderer('markdown').render({ facts: {}, blocks, totalBlocks: 9 });
    assert.ok(
      page.includes('| A |\n| --- |\n| 1 |\n\n…\n\n| B |\n| --- |\n| 2 |'),
      'two tables with a gap line between',
    );
    assert.ok(page.endsWith('| 2 |'), 'no gap after the last row, which is the last of 9');
  });

  it('M2 the index says a truncated page is truncated, in the words the guide uses', () => {
    assert.ok(
      elementIndex([], true).includes(
        '\n⚠ Page content was truncated (very long page); the truncated fact at the top says which part was kept. Scroll and re-analyze to see other parts.\n',
      ),
    );
  });

  it('M3 markdown cut never exceeds max, and its note counts exactly the blocks written', () => {
    const blocks: Block[] = [];
    for (let i = 0; i < 3000; i++) {
      const kind = ['text', 'item', 'row', 'link'][i % 4];
      const screen = i >= 1400 && i < 1450;
      const region = `r${i % 7}`;
      blocks.push(
        kind === 'link'
          ? { id: i + 1, region, kind, text: `b${i}`, target: '/m', state: '', screen }
          : { region, kind, text: `b${i}`, screen },
      );
    }
    for (const max of [2000, 5000, 20000, 80000]) {
      const text = createRenderer('markdown').cut({ facts: { url: 'https://d.test/' }, blocks, elements: [] }, max);
      const markers = text.match(/\bb\d+\b/g) || [];
      assert.ok(text.length <= max, `max ${max}`);
      assert.equal(Number(/showing (\d+) of 3000/.exec(text)?.[1]), markers.length, `max ${max}`);
      const onScreen = markers.filter((m) => Number(m.slice(1)) >= 1400 && Number(m.slice(1)) < 1450).length;
      assert.ok(onScreen > 0, `max ${max} keeps screen rows`);
      if (max >= 5000) assert.equal(onScreen, 50, `max ${max} keeps every screen row`);
    }
  });

  it('markdown falls back to the safe size when gaps overflow the tight one', () => {
    // Gaps between the on-screen rows add blank lines and region comments the tight size does not count.
    // The regions change every two blocks: with a change on every block, this page fits the tight size.
    const kinds = ['text', 'item', 'row', 'link'];
    const blocks: Block[] = [];
    for (let i = 0; i < 400; i++) {
      const kind = kinds[i % 4];
      const region = Math.floor(i / 2) % 2 ? 'aside' : 'main';
      const screen = i % 9 === 0;
      const text = `b${i}`;
      blocks.push(
        kind === 'link'
          ? { id: i + 1, region, kind, text, target: '/m', state: '', screen }
          : { region, kind, text, screen },
      );
    }
    const text = createRenderer('markdown').cut({ facts: { url: 'https://d.test/' }, blocks, elements: [] }, 3000);
    const markers = text.match(/\bb\d+\b/g) || [];
    assert.ok(text.length <= 3000, `length ${text.length}`);
    assert.ok(!text.includes('⚠ Output truncated'), 'the fit safety net did not run');
    assert.equal(Number(/showing (\d+) of 400/.exec(text)?.[1]), markers.length);
  });

  it('J1 the JSONL block lines carry no screen or index key', () => {
    const blocks: Block[] = [
      // Indices 0 and 1, so no gap line is written: this test is about the block lines alone.
      { region: 'main', kind: 'text', text: 'screen index', screen: true, index: 0 },
      { id: 2, region: 'main', kind: 'link', text: 'Link', target: '/l', state: '', screen: true, index: 1 },
    ];
    const lines = createRenderer('jsonl')
      .render({ facts: { url: 'https://d.test/' }, blocks })
      .split('\n')
      .slice(1);
    assert.equal(lines.length, 2);
    for (const line of lines) {
      const block = JSON.parse(line);
      assert.ok(!('screen' in block) && !('index' in block), line);
    }
  });

  it('G1 each guide says what a truncated page means, in the same words', () => {
    assert.ok(
      createRenderer('markdown').guide.includes(
        'truncated, meaning a long page was cut and it says which part was kept, around what is on screen or from the top; scroll and analyze again for another part',
      ),
    );
    assert.ok(
      createRenderer('jsonl').guide.includes(
        'truncated (lines left out; it says which part was kept, around what is on screen or from the top; scroll and analyze again for another part)',
      ),
    );
    assert.ok(
      createRenderer('toon').guide.includes(
        'truncated (rows left out; it says which part was kept, around what is on screen or from the top; scroll and analyze again for another part)',
      ),
    );
  });
});

describe('markdown cut of a block bigger than the budget', () => {
  const bigCode = (screen: boolean): Block => ({
    kind: 'code',
    region: 'main',
    text: 'c'.repeat(30000),
    screen,
    index: 0,
  });

  // The start shown is 5000, not 15000: a block may take half the room (PAGE_RENDER.MAX_BLOCK_SHARE), about 9,900 at 20,000.
  it('R1 markdown keeps the start of an on-screen block too big for the budget', () => {
    const analysis = { facts: { url: 'https://d.test/' }, blocks: [bigCode(true)], totalBlocks: 1, elements: [] };
    const text = createRenderer('markdown').forAgent(analysis, 20000);
    assert.ok(text.length <= 20000, `length ${text.length}`);
    assert.ok(text.includes('c'.repeat(5000)), 'the start of the block is shown');
  });

  it('R1 without the screen mark: markdown keeps the start of a block too big for the budget', () => {
    const analysis = { facts: { url: 'https://d.test/' }, blocks: [bigCode(false)], totalBlocks: 1, elements: [] };
    const text = createRenderer('markdown').forAgent(analysis, 20000);
    assert.ok(text.length <= 20000, `length ${text.length}`);
    assert.ok(text.includes('c'.repeat(5000)), 'the start of the block is shown');
  });

  // Renamed from "R2 an unanchored markdown cut fills the budget past a big block": R2 pinned the removed
  // stopped-block rule (15000 characters and "showing 2 of 3"); R6 is the same page under the clipping rule.
  it('R6 unanchored markdown: a big block is clipped, the blocks around it stay, the note counts what is shown', () => {
    const blocks: Block[] = [
      { kind: 'text', region: 'main', text: 'Home' },
      { kind: 'code', region: 'main', text: 'c'.repeat(30000) },
      { kind: 'text', region: 'main', text: 'Footer' },
    ];
    const text = createRenderer('markdown').cut({ facts: { url: 'https://d.test/' }, blocks, elements: [] }, 20000);
    assert.ok(text.length <= 20000, `length ${text.length}`);
    assert.ok(text.includes('c'.repeat(5000)), 'the start of the big block is shown');
    const shown = ['Home', 'c'.repeat(5000), 'Footer'].filter((part) => text.includes(part)).length;
    assert.match(text, new RegExp(`showing ${shown} of 3 blocks from the top`));
  });

  it('R3 anchored: a block too big for the room shows its start beside the screen, not the top', () => {
    const blocks: Block[] = [
      { kind: 'code', region: 'main', text: 'c'.repeat(3000), index: 0 },
      { kind: 'code', region: 'main', text: 'd'.repeat(3000), screen: true, index: 1 },
    ];
    const text = createRenderer('markdown').cut({ facts: { url: 'https://d.test/' }, blocks, elements: [] }, 2000);
    assert.ok(text.length <= 2000, `length ${text.length}`);
    assert.ok(text.includes('d'.repeat(500)), 'the on-screen block shows its start');
  });

  it('R4 toon: one block bigger than the budget shows its start and is counted as the one block', () => {
    const analysis = {
      facts: { url: 'https://d.test/' },
      blocks: [{ kind: 'code', region: 'main', text: 'c'.repeat(100000) }],
      totalBlocks: 1,
      elements: [],
    };
    const text = createRenderer('toon').forAgent(analysis, 20000);
    assert.ok(text.length <= 20000, `length ${text.length}`);
    assert.ok(text.includes('c'.repeat(5000)), 'the start of the block is shown');
    assert.match(text, /showing 1 of 1 blocks from the top/);
  });

  it('R5 markdown fills the budget with a long list, and its note counts the blocks written', () => {
    const blocks: Block[] = Array.from({ length: 6000 }, (_, i) => ({
      kind: 'item',
      region: 'form/Shipping address billing details',
      text: 'x'.repeat(22) + i,
      screen: i === 3000,
      index: i,
    }));
    const text = createRenderer('markdown').cut({ facts: { url: 'https://d.test/' }, blocks, elements: [] }, 80000);
    assert.ok(text.length >= 76000 && text.length <= 80000, `length ${text.length}`);
    const written = (text.match(/x{22}\d+/g) || []).length;
    assert.equal(Number(/showing (\d+) of 6000/.exec(text)?.[1]), written);
  });
});

describe('gap rows and clipping in TOON and JSONL', () => {
  /** The analyzer-capped page: rows 0..149 and 5000..5149 of 9000, the row at 5050 on screen. */
  const cappedRows = (): Block[] => {
    const row = (i: number): Block => ({
      region: 'main',
      kind: 'text',
      text: `Para ${i} ` + 'x'.repeat(200),
      index: i,
      screen: i === 5050,
    });
    const rows: Block[] = [];
    for (let i = 0; i < 150; i++) rows.push(row(i));
    for (let i = 5000; i < 5150; i++) rows.push(row(i));
    return rows;
  };

  it('G2 TOON writes a gap row where rows were left out, and its header counts every row written', () => {
    const analysis = { facts: { url: 'https://d.test/' }, blocks: cappedRows(), totalBlocks: 9000, elements: [] };
    const text = createRenderer('toon').forAgent(analysis, 20000);
    const lines = text.split('\n');
    const gap = lines.findIndex((line) => /^ {2}"",""?,gap,/.test(line));
    const firstRow = lines.findIndex((line) => line.includes('Para '));
    assert.ok(gap >= 0, 'a gap row is written');
    assert.ok(gap < firstRow, 'the gap sits before the first kept row');
    assert.ok(text.includes('Para 5050 '), 'the on-screen row is kept');
    const rows = lines.filter((line) => /^ {2}[^ ]/.test(line) && !/^ {2}\w+: /.test(line));
    assert.equal(Number(/blocks\[(\d+)\]/.exec(text)?.[1]), rows.length);
  });

  it('G2 JSONL writes a gap line where lines were left out', () => {
    const analysis = { facts: { url: 'https://d.test/' }, blocks: cappedRows(), totalBlocks: 9000, elements: [] };
    const lines = createRenderer('jsonl').forAgent(analysis, 20000).split('\n');
    assert.ok(lines.includes('{"kind":"gap"}'), 'a gap line is written');
    assert.ok(
      lines.some((line) => line.includes('Para 5050 ')),
      'the on-screen row is kept',
    );
  });

  it('C2 TOON shows the start of one block whose escaped text is bigger than the budget', () => {
    const analysis = {
      facts: { url: 'https://d.test/' },
      blocks: [{ kind: 'code', text: '"'.repeat(60000) }],
      elements: [],
    };
    const text = createRenderer('toon').forAgent(analysis, 20000);
    assert.ok(text.length <= 20000, `length ${text.length}`);
    assert.match(text, /showing 1 of 1 blocks/);
  });

  it('C3 a cut that falls inside an emoji leaves no lone surrogate', () => {
    // 9001 + 2 x 6000 units: over the budget, so the block is clipped, and with an odd prefix the clip falls inside an emoji.
    const analysis = {
      facts: { url: 'https://d.test/' },
      blocks: [{ kind: 'text', text: 'a'.repeat(9001) + '😀'.repeat(6000) }],
      elements: [],
    };
    for (const format of ['toon', 'jsonl', 'markdown']) {
      const text = createRenderer(format).forAgent(analysis, 20000);
      assert.doesNotMatch(text, /[\uD800-\uDBFF](?![\uDC00-\uDFFF])/, format);
    }
  });
});
