/**
 * Unit tests for scripts/workflow/chrome-recorder.cjs: a Chrome DevTools
 * Recorder recording opens as a workflow that generates, and a workflow saves
 * in Chrome's shape.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const {
  isChromeRecording,
  fromChromeRecording,
  toChromeRecording,
} = require('../../../../scripts/workflow/chrome-recorder.cjs');
const { normalizeDraft, generate } = require('../../../../scripts/workflow.cjs');

/** A recording as Chrome's Recorder panel saves one. */
const CHROME = {
  title: 'Log in',
  steps: [
    { type: 'setViewport', width: 1280, height: 720 },
    {
      type: 'navigate',
      url: 'https://x.test/login',
      assertedEvents: [{ type: 'navigation', url: 'https://x.test/login' }],
    },
    { type: 'click', selectors: [['aria/Email'], ['#email'], ['xpath///*[@id="email"]']], offsetX: 5, offsetY: 5 },
    { type: 'change', value: 'ann@x.test', selectors: [['aria/Email'], ['#email']] },
    { type: 'keyDown', key: 'Enter' },
    { type: 'keyUp', key: 'Enter' },
    {
      type: 'click',
      selectors: [['pierce/#submit']],
      frame: [0],
      assertedEvents: [{ type: 'navigation', url: 'https://x.test/home?session=1' }],
    },
    { type: 'doubleClick', selectors: [['text/Row 1']] },
    { type: 'hover', selectors: [['.menu']] },
    { type: 'scroll', x: 0, y: -300 },
  ],
};

describe('Chrome Recorder JSON', () => {
  it('knows a Chrome recording from an Oya workflow', () => {
    assert.equal(isChromeRecording(CHROME), true);
    assert.equal(isChromeRecording({ steps: [{ action: 'click' }] }), false);
  });

  it('opens a Chrome recording as steps a workflow runs, leaving out what it has no step for', () => {
    const draft = normalizeDraft(fromChromeRecording(CHROME));
    assert.equal(draft.name, 'Log in');
    assert.deepEqual(
      draft.steps.map((s) => s.action),
      ['navigate', 'click', 'type', 'press_key', 'click', 'assert_page', 'double_click', 'hover', 'scroll'],
    );
    generate(draft);
  });

  it('keeps Chrome’s selector order with a positional path last, and reads xpath, pierce, text and a frame index', () => {
    const [, click, type, , framed, check, double] = normalizeDraft(fromChromeRecording(CHROME)).steps;
    assert.deepEqual(
      click.candidates.map((c) => c.value),
      ['Email', '#email', 'xpath=//*[@id="email"]'],
    );
    assert.equal(type.candidates[0].kind, 'label', 'a name on a field is its label');
    assert.deepEqual(framed.frames, ['iframe >> nth=0']);
    assert.equal(check.expected, 'https://x.test/home?session=1');
    assert.deepEqual(double.candidates, [{ kind: 'text', value: 'Row 1' }]);
  });

  it('saves a workflow as Chrome steps: keys pressed and let go, a page check as a wait on the address', () => {
    const draft = normalizeDraft({
      name: 'flow',
      steps: [
        { action: 'navigate', url: 'https://x.test/' },
        { action: 'type', text: 'ann', candidates: [{ kind: 'label', value: 'Name' }] },
        { action: 'press_key', key: 'Enter' },
        { action: 'assert_page', expected: 'https://x.test/done?s=1' },
        { action: 'checkpoint' },
      ],
    });
    const chrome = toChromeRecording(draft);
    assert.deepEqual(
      chrome.steps.map((s) => s.type),
      ['navigate', 'change', 'keyDown', 'keyUp', 'waitForExpression'],
    );
    assert.deepEqual(chrome.steps[1].selectors, [['aria/Name']]);
    assert.match(chrome.steps[4].expression, /"https:\/\/x\.test\/done"$/);
  });

  it('comes back the same through Chrome for the steps both have', () => {
    const steps = [
      { action: 'navigate', url: 'https://x.test/' },
      { action: 'click', candidates: [{ kind: 'css', value: '#go' }] },
      { action: 'hover', candidates: [{ kind: 'text', value: 'Menu' }] },
    ];
    const back = normalizeDraft(fromChromeRecording(toChromeRecording(normalizeDraft({ steps }))));
    assert.deepEqual(
      back.steps.map((s) => [s.action, s.url, s.candidates[0]?.value]),
      [
        ['navigate', 'https://x.test/', undefined],
        ['click', undefined, '#go'],
        ['hover', undefined, 'Menu'],
      ],
    );
  });

  it('saves Back as a navigation to where it landed, and leaves out selectors only Playwright reads', () => {
    const draft = normalizeDraft({
      steps: [
        { action: 'navigate', url: 'https://x.test/a' },
        {
          action: 'click',
          candidates: [
            { kind: 'css', value: '[id="t"] a:text-is("See")' },
            { kind: 'text', value: 'See' },
          ],
        },
        { action: 'go_back' },
        { action: 'assert_page', expected: 'https://x.test/a' },
      ],
    });
    const chrome = toChromeRecording(draft);
    assert.deepEqual(chrome.steps[1].selectors, [['text/See']]);
    assert.deepEqual(chrome.steps[2], { type: 'navigate', url: 'https://x.test/a' });
  });

  it('opens its own page checks back as page checks', () => {
    const draft = normalizeDraft({ steps: [{ action: 'assert_page', expected: 'https://x.test/done?s=1' }] });
    const back = fromChromeRecording(toChromeRecording(draft));
    assert.deepEqual(back.steps, [{ action: 'assert_page', expected: 'https://x.test/done' }]);
  });
});
