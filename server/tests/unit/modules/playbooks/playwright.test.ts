/**
 * Unit tests for the Playwright export: the source generated for each step
 * type, locator precedence, placeholders and filters, defaults without
 * secrets, the dialog handler, and version-2 workflows.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { renderPlaywright } from '../../../../src/modules/playbooks/playwright.ts';
import { DEFAULT_SCROLL_PX } from '../../../../src/modules/playbooks/constants.ts';

/** The export of a playbook with these steps. */
const render = (steps, extra = {}) => renderPlaywright({ name: 'demo', steps, ...extra });
/** The generated line for one step. */
const lineFor = (step, extra = {}) =>
  render([step], extra)
    .split('\n')
    .find((l) => l.startsWith('  ') && !l.startsWith('  vars ='))
    .trim();

describe('renderPlaywright', () => {
  describe('steps', () => {
    it('navigates to a recorded URL', () => {
      assert.equal(lineFor({ action: 'navigate', url: 'https://a.test/' }), 'await page.goto("https://a.test/");');
    });

    it('hovers over the recorded element', () => {
      assert.equal(
        lineFor({ action: 'hover', el: { testId: 'menu' } }),
        'await page.getByTestId("menu").first().hover();',
      );
    });

    it('moves through history as the run did', () => {
      assert.equal(lineFor({ action: 'go_back' }), 'await page.goBack();');
      assert.equal(lineFor({ action: 'go_forward' }), 'await page.goForward();');
      assert.equal(lineFor({ action: 'reload' }), 'await page.reload();');
    });

    it('clicks the first element a locator finds', () => {
      assert.equal(lineFor({ action: 'click', el: { testId: 'go' } }), 'await page.getByTestId("go").first().click();');
    });

    it('fills a field, finding it by its label text', () => {
      assert.equal(
        lineFor({ action: 'type', text: 'hello', el: { text: 'Name' } }),
        'await page.getByLabel("Name").first().fill("hello");',
      );
    });

    it('fills an empty string when a type step recorded no text', () => {
      assert.match(lineFor({ action: 'type', el: { testId: 't' } }), /\.fill\(""\);$/);
    });

    it('selects an option by its label', () => {
      assert.equal(
        lineFor({ action: 'select_option', option: 'Blue', el: { name: 'color', tag: 'select' } }),
        'await page.locator("select[name=\\"color\\"]").first().selectOption({ label: "Blue" });',
      );
    });

    it('uploads through the file input itself, not the recorded button', () => {
      assert.equal(
        lineFor({ action: 'upload_file', file: '{{resume}}', el: { text: 'Upload' } }),
        'await page.locator(\'input[type="file"]\').first().setInputFiles(`${vars["resume"]}`);',
      );
    });

    it('presses a key', () => {
      assert.equal(lineFor({ action: 'press_key', key: 'Enter' }), 'await page.keyboard.press("Enter");');
    });

    it('waits for a selector, with its timeout when one was recorded', () => {
      assert.equal(lineFor({ action: 'wait', selector: '#x' }), 'await page.waitForSelector("#x");');
      assert.equal(
        lineFor({ action: 'wait', selector: '#x', timeout: '5000' }),
        'await page.waitForSelector("#x", { timeout: 5000 });',
      );
    });

    it('scrolls to the top, to the bottom, or by an amount in either direction', () => {
      assert.equal(
        lineFor({ action: 'scroll', direction: 'top' }),
        'await page.evaluate(() => window.scrollTo(0, 0));',
      );
      assert.match(lineFor({ action: 'scroll', direction: 'bottom' }), /scrollHeight/);
      assert.equal(lineFor({ action: 'scroll', direction: 'up', amount: 300 }), 'await page.mouse.wheel(0, -300);');
      assert.equal(
        lineFor({ action: 'scroll', direction: 'down' }),
        `await page.mouse.wheel(0, ${DEFAULT_SCROLL_PX});`,
      );
    });

    it('comments an answered dialog instead of acting on it', () => {
      const lines = render([{ action: 'handle_dialog' }]).split('\n');
      assert.equal(lines.at(-3), '  // dialog answered by the handler above');
    });

    it('comments out an unknown action, including inherited property names', () => {
      assert.equal(lineFor({ action: 'teleport' }), '// skipped unknown step "teleport"');
      assert.equal(lineFor({ action: 'toString' }), '// skipped unknown step "toString"');
      assert.equal(lineFor({ action: 42 }), '// skipped unknown step 42');
    });
  });

  describe('locators', () => {
    const click = (el) => lineFor({ action: 'click', el }).replace(/^await (.*)\.first\(\)\.click\(\);$/, '$1');

    it('prefers a test id over every other handle', () => {
      assert.equal(click({ testId: 't', domId: 'd', text: 'x' }), 'page.getByTestId("t")');
    });

    it('then the link target, the written id, the label, the text, the name, the placeholder and the position', () => {
      // A link's own target outranks an id: the id may be this render's invention.
      assert.equal(click({ tag: 'a', href: '/x', domId: 'd' }), 'page.locator("a[href=\\"/x\\"]")');
      assert.equal(click({ domId: 'd', ariaLabel: 'a' }), 'page.locator("[id=\\"d\\"]")');
      assert.equal(click({ ariaLabel: 'a', text: 'x' }), 'page.getByLabel("a", { exact: true })');
      assert.equal(click({ text: 'Go', name: 'n' }), 'page.getByText("Go", { exact: true })');
      assert.equal(click({ name: 'n', tag: 'input', placeholder: 'p' }), 'page.locator("input[name=\\"n\\"]")');
      assert.equal(click({ name: 'n' }), 'page.locator("[name=\\"n\\"]")');
      assert.equal(click({ placeholder: 'p', path: 'form > input' }), 'page.getByPlaceholder("p")');
      assert.equal(click({ path: 'div > p > a' }), 'page.locator("div > p > a")');
    });

    it('passes over a link target that carries the request id of the visit it was recorded in', () => {
      // Recorded from Amazon's page-2 link. As an attribute selector it matches the
      // visit it was recorded in and nothing after it, so the label takes over.
      const el = { tag: 'a', rawHref: '/s?k=kb&page=2&qid=1789940643&xpid=njio9', ariaLabel: 'Go to page 2' };
      assert.equal(click(el), 'page.getByLabel("Go to page 2", { exact: true })');
    });

    it('aims a repeated name at the container where it occurs once', () => {
      const scoped = '[data-row="7"] button:text-is("View")';
      // Three rows with a View button each: the name alone clicks the first row.
      assert.equal(
        click({ text: 'View', repeats: 'true', scoped, tag: 'button' }),
        `page.locator(${JSON.stringify(scoped)})`,
      );
    });

    it('falls to position for a repeated name no container could pin down', () => {
      assert.equal(
        click({ text: 'Delete', repeats: 'true', tag: 'button', path: 'li:nth-of-type(2) > button' }),
        'page.locator("li:nth-of-type(2) > button")',
      );
    });

    it('never aims at an id a framework invented for that render', () => {
      // The failure this came from: a recorded Send button whose only id was
      // Ember's, which belongs to a different element on the next render.
      assert.equal(click({ domId: 'ember80', text: 'Send', tag: 'button' }), 'page.getByText("Send", { exact: true })');
      assert.equal(click({ domId: 'mwCg', path: 'p > a' }), 'page.locator("p > a")');
    });

    it('drops a live count from a label, so a nav link survives a notification', () => {
      assert.equal(
        click({ ariaLabel: 'Messaging, 0 new notifications', tag: 'a' }),
        'page.getByLabel("Messaging", { exact: true })',
      );
    });

    it('refuses a step nothing can aim, rather than clicking the page body', () => {
      // A body click runs, does nothing, and lets the rest of the script continue
      // as though the step had worked, the worst of the three outcomes.
      const line = lineFor({ action: 'click', el: { tag: 'button' } });
      assert.match(line, /^throw new Error\("Cannot replay this click: no stable handle/);
      assert.match(lineFor({ action: 'click' }), /no stable handle was recorded for an element/);
      assert.match(lineFor({ action: 'type', text: 'x', el: {} }), /Cannot replay this type/);
    });

    it('still runs a step that needs no element', () => {
      assert.equal(lineFor({ action: 'press_key', key: 'Tab' }), 'await page.keyboard.press("Tab");');
      assert.match(lineFor({ action: 'scroll', direction: 'down' }), /mouse\.wheel/);
    });

    it('refuses a step the recorder marked unaimable', () => {
      assert.match(lineFor({ action: 'click', el: { testId: 'go' }, unaimable: true }), /^throw new Error/);
    });

    it('finds a data-driven click by its filled-in text before any id', () => {
      assert.equal(click({ testId: 't', text: '{{plan}}' }), 'page.getByText(`${vars["plan"]}`, { exact: true })');
    });
  });

  describe('placeholders', () => {
    it('turns a placeholder into a template literal reading vars', () => {
      assert.equal(
        lineFor({ action: 'navigate', url: 'https://a.test/?q={{query}}' }),
        'await page.goto(`https://a.test/?q=${vars["query"]}`);',
      );
    });

    it('escapes backticks, dollars and backslashes around a placeholder', () => {
      assert.equal(
        lineFor({ action: 'type', text: '`$\\{{x}}', el: { testId: 't' } }),
        'await page.getByTestId("t").first().fill(`\\`\\$\\\\${vars["x"]}`);',
      );
    });

    it('applies filters through v() and ships the filter helpers only when used', () => {
      const code = render([{ action: 'type', text: '{{name|first}}', el: { testId: 't' } }]);
      assert.match(code, /fill\(`\$\{v\(vars, "name", \[\["first"\]\]\)\}`\)/);
      assert.match(code, /^const FILTERS = \{$/m);
      assert.match(code, /^const v = /m);
      assert.doesNotMatch(render([{ action: 'type', text: '{{name}}', el: { testId: 't' } }]), /FILTERS/);
    });

    it('produces filter helpers that run', () => {
      const code = render([{ action: 'type', text: '{{d|date:YYYY}}', el: { testId: 't' } }]);
      const helpers = code
        .split('\n')
        .filter((l) => !l.startsWith('//'))
        .join('\n')
        .split('export default')[0];
      const v = new Function(`${helpers}; return v;`)();
      assert.equal(v({ d: '2024-03-05' }, 'd', [['date', 'DD/MM/YYYY']]), '05/03/2024');
      assert.equal(v({ n: 'Ada Lovelace' }, 'n', [['last'], ['upper'], ['nope']]), 'LOVELACE');
    });
  });

  describe('module', () => {
    it('names the playbook and its variables in the header', () => {
      const code = render([
        { action: 'navigate', url: '{{a}}' },
        { action: 'type', text: '{{b}}', el: { testId: 't' } },
      ]);
      assert.match(code, /^\/\/ Playbook "demo", generated by Oya\.\n\/\/ vars: a, b\n/);
      assert.match(render([{ action: 'press_key', key: 'Tab' }]), /^\/\/ vars: \(none\)$/m);
    });

    it('notes that upload variables are file paths here', () => {
      assert.match(
        render([{ action: 'upload_file', file: '{{cv}}' }]),
        /^\/\/ here cv is a file path, not an Oya file\(\) value$/m,
      );
      const two = render([
        { action: 'upload_file', file: '{{cv}}' },
        { action: 'upload_file', file: '{{photo}}' },
      ]);
      assert.match(two, /^\/\/ here cv, photo are file paths/m);
      assert.doesNotMatch(render([{ action: 'upload_file' }]), /here/);
    });

    it('bakes in the defaults the steps use, but never a secret', () => {
      const code = render([{ action: 'type', text: '{{user}} {{pass}}', el: { testId: 't' } }], {
        defaults: { user: 'ada', pass: 'hunter2', unused: 'x' },
        secrets: ['pass'],
      });
      assert.match(code, /^ {2}vars = \{ \.\.\.\{"user":"ada"\}, \.\.\.vars \};$/m);
      assert.doesNotMatch(code, /hunter2/);
    });

    it('answers dialogs the way the recording did, with the prompt text it typed', () => {
      const code = render([{ action: 'handle_dialog', prompt_text: 'yes' }]);
      assert.match(code, /^ {2}page\.on\('dialog', \(d\) => d\.accept\("yes"\)\);$/m);
      assert.match(render([{ action: 'handle_dialog' }]), /d\.accept\(\)\);/);
    });

    it('installs no dialog handler when the recording dismissed its dialogs', () => {
      assert.doesNotMatch(render([{ action: 'handle_dialog', accept: false }]), /page\.on/);
    });

    it('exports a run function wrapping the steps, ending with a newline', () => {
      const code = render([{ action: 'press_key', key: 'Tab' }]);
      assert.match(code, /export default async function run\(page, vars = \{\}\) \{\n[\s\S]*\n\}\n$/);
    });

    it('hands a version-2 workflow to the browser’s generator', () => {
      const code = renderPlaywright({
        schemaVersion: 2,
        name: 'wf',
        steps: [{ action: 'navigate', url: 'https://a.test/{{q}}' }],
        variables: { q: { default: 'x' } },
      });
      assert.match(code, /^\/\/ Generated by Oya\. Optional hooks/);
      assert.match(code, /"q":"x"/);
    });
  });
  it('exports every action a run can take, so no step goes missing', () => {
    const steps = [
      { action: 'navigate', url: 'https://portal.example.com/web/auth' },
      { action: 'keyboard_type', text: 'MEM000000001' },
      { action: 'click_coordinates', x: 412, y: 688 },
      { action: 'switch_tab', tabUrl: 'https://vendor.example.com/order/new?ssoToken=abc123' },
      { action: 'close_tab', tabUrl: 'https://vendor.example.com/order/new?ssoToken=abc123' },
    ];
    const code = renderPlaywright({ name: 'preauth', prompt: 'p', defaults: {}, secrets: [], steps });
    assert.equal(code.includes('skipped unknown'), false);
    assert.match(code, /await page\.keyboard\.type\("MEM000000001"\)/);
    assert.match(code, /await page\.mouse\.click\(412, 688\)/);
    // The handoff tab is found by origin and path: its token differs every run.
    assert.match(code, /tabAt\(context, "https:\/\/vendor\.example\.com\/order\/new"\)/);
    assert.equal(code.includes('ssoToken'), false);
  });

  it('adds the tab finder only when a run used more than one tab', () => {
    const single = renderPlaywright({
      name: 'one',
      prompt: 'p',
      defaults: {},
      secrets: [],
      steps: [{ action: 'press_key', key: 'Tab' }],
    });
    assert.equal(single.includes('tabAt'), false);
  });
});
