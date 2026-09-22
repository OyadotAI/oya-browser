/**
 * Unit tests for what each browser tool does and tells the model, against a
 * scripted browser: the command it sends, its answer, and its error text.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { TOOL_HANDLERS } = await import('../../../../src/modules/agent/tool-handlers.ts');
const { BROWSER_TOOLS } = await import('../../../../src/modules/agent/tools.ts');
const recorder = await import('../../../../src/modules/agent/recorder.ts');
const { MAX_ANALYSIS_CHARS, NAVIGATE_TIMEOUT_MS } = await import('../../../../src/modules/agent/constants.ts');
const { scriptedBrowser } = await import('../../support/agent.ts');

const BROWSER = 'b-tools';
const FILE = { file: 'cv.pdf', type: 'application/pdf', b64: 'AAAA' };
let answer: (action, params) => any;
let browser;

/** Runs one tool. */
const run = (name, args = {}, files = {}) => TOOL_HANDLERS[name](BROWSER, args, files);
/** The last command the browser received. */
const lastCall = () => browser.calls.at(-1);
/** The last command of one kind, past the analyze an action answers with. */
const lastCallOf = (action) => browser.calls.filter((c) => c.action === action).at(-1);

describe('tool handlers', () => {
  beforeEach(async () => {
    answer = () => ({ ok: true, data: {} });
    // An Oya browser: the model is offered read_console and read_network, which only an Oya browser does.
    browser = scriptedBrowser(BROWSER, 'key-a', (a, p) => answer(a, p), 'oya');
    await recorder.startRun(BROWSER, { steps: [], elements: [] });
  });
  afterEach(() => browser.disconnect());

  it('has a handler for every tool offered to the model, and no more', () => {
    // The loop answers screenshot itself: its result is an image, not text.
    const offered = BROWSER_TOOLS.map((t) => t.function.name).filter((n) => n !== 'screenshot');
    assert.deepEqual(Object.keys(TOOL_HANDLERS).sort(), offered.sort());
  });

  it('answers every tool’s browser error as Error text', async () => {
    answer = () => ({ ok: false, error: 'boom' });
    const args = { element_id: 1, option: 'x', url: 'https://a.test', selector: '#a', tab_id: 1, x: 1, y: 2 };
    recorder.setElements(BROWSER, [{ id: 1 }]);
    for (const name of Object.keys(TOOL_HANDLERS)) {
      const out = await run(name, { ...args, name: 'cv' }, { cv: FILE });
      assert.match(out, /^Error: boom/, name);
    }
  });

  describe('analyze_page', () => {
    it('returns the page with its element index and keeps the elements for later steps', async () => {
      const elements = [{ id: 4, type: 'button', text: 'Go', visible: true }];
      answer = () => ({ ok: true, data: { markdown: '# Page', elements, truncated: false } });
      const out = await run('analyze_page');
      assert.match(out, /^# Page\n\n## Element Index \(1 total, 1 visible\)/);
      assert.equal(recorder.elementOf(BROWSER, 4), elements[0]);
    });

    it('cuts the markdown of an analysis that would fill the context window, keeping the element index whole', async () => {
      const elements = [{ id: 7, type: 'button', text: 'Next', visible: true }];
      const markdown = 'line of page text\n'.repeat(MAX_ANALYSIS_CHARS / 8);
      answer = () => ({ ok: true, data: { markdown, elements } });
      const out = await run('analyze_page');
      assert.ok(out.length <= MAX_ANALYSIS_CHARS);
      assert.match(out, /line of page text\n\n⚠ Output truncated to fit context window\.\n\n## Element Index/);
      assert.ok(out.includes('7,button,Next'));
    });
  });

  it('navigates, waiting as long as a slow site needs, and answers with the page it landed on', async () => {
    answer = (a) =>
      a === 'analyze'
        ? { ok: true, data: { elements: [{ id: 2, type: 'link', text: 'Next' }] } }
        : { ok: true, data: {} };
    const out = await run('navigate', { url: 'https://a.test' });
    // The elements come back with the navigation, so no analyze_page call is needed to act.
    assert.match(out, /^Navigated to https:\/\/a\.test/);
    assert.match(out, /Next/);
    assert.deepEqual(lastCallOf('navigate'), {
      action: 'navigate',
      params: { url: 'https://a.test' },
      timeout: NAVIGATE_TIMEOUT_MS,
    });
  });

  it('clicks and types by the element id’s selector', async () => {
    assert.equal(await run('click', { element_id: 3 }), 'Clicked element 3');
    assert.deepEqual(lastCallOf('click').params, { selector: '[data-ac-id="3"]' });
    assert.equal(await run('type', { element_id: 3, text: 'hi' }), 'Typed "hi" into element 3');
    assert.deepEqual(lastCallOf('type').params, { selector: '[data-ac-id="3"]', text: 'hi' });
  });

  it('answers an action with the elements it left behind, so the model need not read the page again', async () => {
    answer = (action) =>
      action === 'analyze'
        ? {
            ok: true,
            data: {
              format: 'markdown',
              facts: {},
              blocks: [],
              elements: [{ id: 7, type: 'button', text: 'Save', visible: true }],
            },
          }
        : { ok: true, data: {} };
    const said = await run('click', { element_id: 3 });
    assert.match(said, /^Clicked element 3/);
    assert.match(said, /Save/, 'the elements now on the page come back with it');
  });

  it('says which value a date input now holds', async () => {
    answer = () => ({ ok: true, data: { typed: true, value: '2024-01-15' } });
    assert.equal(await run('type', { element_id: 3, text: '01/15/2024' }), 'Set element 3 to 2024-01-15');
  });

  it('says what the field shows when it is not what was typed', async () => {
    answer = () => ({ ok: true, data: { typed: true, shown: '02/32/026_' } });
    assert.equal(
      await run('type', { element_id: 69, text: '10/23/2026' }),
      'Typed "10/23/2026" into element 69, but the field now shows "02/32/026_". If that is wrong, type the whole value again.',
    );
  });

  it('points the model at autocomplete suggestions after typing', async () => {
    answer = () => ({ ok: true, data: { suggestions_visible: true } });
    assert.match(await run('type', { element_id: 3, text: 'Ber' }), /AUTOCOMPLETE SUGGESTIONS ARE VISIBLE/);
  });

  it('presses a key', async () => {
    assert.equal(await run('press_key', { key: 'Enter' }), 'Pressed Enter');
  });

  it('does not resend the element index when the click left the page as it was', async () => {
    const elements = [{ id: 1, type: 'button', text: 'Go', visible: true }];
    answer = () => ({ ok: true, data: { elements } });
    recorder.setElements(BROWSER, elements);
    const out = await run('click', { element_id: 1 });
    assert.match(out, /ids you were given still work/);
    assert.doesNotMatch(out, /Element Index/);
    // A page that did move brings its new ids with it.
    answer = () => ({ ok: true, data: { elements: [{ id: 1, type: 'button', text: 'Next', visible: true }] } });
    assert.match(await run('click', { element_id: 1 }), /Element Index/);
  });

  describe('select_option', () => {
    it('refuses an element id that is not in the latest analysis', async () => {
      assert.equal(
        await run('select_option', { element_id: 99, option: 'x' }),
        'Error: Element not found. Call analyze_page and use a current id.',
      );
    });

    it('chooses the option in the page and names what it chose', async () => {
      recorder.setElements(BROWSER, [{ id: 5, name: 'color' }]);
      answer = () => ({ ok: true, data: { result: { ok: true, chosen: 'Blue' } } });
      assert.equal(await run('select_option', { element_id: 5, option: 'blue' }), 'Selected "Blue" in element 5');
      assert.equal(lastCallOf('evaluate_raw').action, 'evaluate_raw');
    });

    it('lists the options when none matches', async () => {
      recorder.setElements(BROWSER, [{ id: 5 }]);
      answer = () => ({
        ok: true,
        data: { result: { ok: false, error: 'no single option matches', options: ['Red', 'Blue'] } },
      });
      assert.equal(
        await run('select_option', { element_id: 5, option: 'x' }),
        'Error: no single option matches. Options: Red | Blue',
      );
    });
  });

  describe('upload_file', () => {
    it('refuses a file the task does not have, naming the ones it does', async () => {
      assert.equal(
        await run('upload_file', { name: 'photo' }, { cv: FILE }),
        'Error: no file named "photo" in the task data. Available: cv.',
      );
      assert.equal(
        await run('upload_file', { name: 'photo' }, {}),
        'Error: no file named "photo" in the task data. This task was given no files.',
      );
    });

    it('refuses an element id that is not in the latest analysis', async () => {
      assert.match(await run('upload_file', { name: 'cv', element_id: 9 }, { cv: FILE }), /Element not found/);
    });

    it('attaches the file, taking the name with placeholder braces too', async () => {
      answer = () => ({ ok: true, data: { result: { ok: true, field: 'resume' } } });
      assert.equal(await run('upload_file', { name: '{{cv}}' }, { cv: FILE }), 'Attached cv.pdf to resume');
    });
  });

  it('scrolls and waits', async () => {
    assert.equal(await run('scroll', { direction: 'down', amount: 100 }), 'Scrolled down');
    assert.equal(await run('wait', { selector: '#a', timeout: 10 }), 'Element found: #a');
  });

  it('shows the page a scroll landed on when the browser analysed it', async () => {
    const elements = [{ id: 9, type: 'link', text: 'More', visible: true }];
    answer = () => ({ ok: true, data: { markdown: '# Lower down', elements } });
    const out = await run('scroll', { direction: 'down' });
    assert.match(out, /^Scrolled down\.\n\n# Lower down\n\n## Element Index/);
    assert.equal(recorder.elementOf(BROWSER, 9), elements[0]);
  });

  it('lists the elements with ids click accepts, keeping them for later steps', async () => {
    const elements = [{ id: 3, type: 'button', text: 'Go', visible: true }];
    answer = () => ({ ok: true, data: { url: 'https://a.test', title: 'A', markdown: '# A', elements } });
    const out = await run('read_elements', { selector: 'a' });
    assert.match(out, /^Page: A \(https:\/\/a\.test\)\n\n## Element Index \(1 total, 1 visible\)/);
    assert.ok(out.includes('3,button,Go') && !out.includes('# A'));
    assert.equal(recorder.elementOf(BROWSER, 3), elements[0]);
    assert.deepEqual(browser.calls.at(-1).params, { selector: 'a' });
  });

  it('lists tabs, marking the active one', async () => {
    answer = () => ({
      ok: true,
      data: {
        tabs: [
          { id: 1, title: 'A', url: 'u1', active: true },
          { id: 2, title: 'B', url: 'u2' },
        ],
      },
    });
    assert.equal(await run('list_tabs'), 'Tabs:\n→ [tab 1] A, u1\n  [tab 2] B, u2');
    answer = () => ({ ok: true, data: {} });
    assert.equal(await run('list_tabs'), 'Tabs:\n');
  });

  it('opens, switches and closes tabs', async () => {
    answer = () => ({ ok: true, data: { tab_id: 3 } });
    assert.equal(await run('open_tab', { url: 'https://a.test' }), 'Opened tab 3 at https://a.test');
    answer = () => ({ ok: true });
    assert.equal(await run('open_tab'), 'Opened tab ');
    assert.equal(await run('switch_tab', { tab_id: 3 }), 'Switched to tab 3');
    assert.equal(await run('close_tab', { tab_id: 3 }), 'Closed tab');
  });

  it('acts at coordinates', async () => {
    assert.equal(await run('click_coordinates', { x: 1, y: 2 }), 'Clicked at 1,2');
    assert.equal(await run('mouse_move', { x: 3, y: 4 }), 'Moved the mouse to 3,4');
    assert.equal(await run('drag', { from_x: 1, from_y: 2, to_x: 3, to_y: 4 }), 'Dragged from 1,2 to 3,4');
    assert.equal(await run('keyboard_type', { text: 'hi' }), 'Typed "hi" into the focused element');
  });

  it('double-clicks an element by id, or at coordinates', async () => {
    assert.equal(await run('double_click', { element_id: 2 }), 'Double-clicked element 2');
    assert.deepEqual(lastCall().params, { element_id: 2, selector: '[data-ac-id="2"]' });
    assert.equal(await run('double_click', { x: 5, y: 6 }), 'Double-clicked at 5,6');
    assert.deepEqual(lastCall().params, { x: 5, y: 6 });
  });

  it('accepts or dismisses a dialog', async () => {
    answer = () => ({ ok: true, data: { accepted: false, type: 'confirm' } });
    assert.match(await run('handle_dialog', { accept: false }), /^Dismissed the confirm dialog\./);
    answer = () => ({ ok: true });
    assert.match(await run('handle_dialog', { accept: true }), /^Accepted the {2}dialog\./);
  });
});
