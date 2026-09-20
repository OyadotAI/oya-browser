/**
 * Unit tests for replay without the LLM, against a scripted browser: each step
 * is matched to the live page and sent as a command, the checkpoint runs after
 * page-changing steps, a broken step throws or heals, and version-2 workflows
 * run whole in the browser.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const { play } = await import('../../../../src/modules/playbooks/replay.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');
const { FIND_ATTEMPTS, FIND_RETRY_MS, NAVIGATE_TIMEOUT_MS, REPLAY_PAUSE_MS, REPLAY_SETTLE_MS } =
  await import('../../../../src/modules/playbooks/constants.ts');
const { scriptedBrowser, stubLlm, textReply } = await import('../../support/agent.ts');
const { advance } = await import('../../support/http.ts');

const KEY = 'replay-key';
const BROWSER = 'b-replay';
const ELEMENTS = [
  { id: 1, type: 'input', tag: 'input', name: 'email', visible: true },
  { id: 2, type: 'button', tag: 'button', text: 'Go', visible: true },
  { id: 3, type: 'select', tag: 'select', name: 'plan', visible: true },
  { id: 4, type: 'button', tag: 'button', text: 'Pro', domId: 'plan-basic', visible: true },
];
const FILE = { file: 'cv.pdf', type: 'application/pdf', b64: 'AAAA' };
let answer: (action, params) => any;
let browser;

/** The page's default answers: analyze lists ELEMENTS, scripts succeed. */
const page = (action) => {
  if (action === 'analyze') return { ok: true, data: { elements: ELEMENTS } };
  if (action === 'evaluate_raw') return { ok: true, data: { result: { ok: true, chosen: 'x', field: 'cv' } } };
  return { ok: true, data: {} };
};
/** A playbook of these steps. */
const pb = (steps, extra = {}) => ({ name: 'demo', prompt: 'Do the demo', steps, defaults: {}, ...extra });
/** The commands sent, without the analyses. */
const commands = () => browser.calls.filter((c) => c.action !== 'analyze');

describe('play', () => {
  beforeEach(() => {
    answer = page;
    browser = scriptedBrowser(BROWSER, KEY, (a, p) => answer(a, p));
  });
  afterEach(() => {
    browser.disconnect();
    mock.restoreAll();
    mock.timers.reset();
    keyConfig.reset();
    delete process.env.OPENAI_API_KEY;
    delete process.env.CHAT_MAX_ITERATIONS;
  });

  it('replays every step type as its browser command, filling values', async () => {
    const steps = [
      { action: 'navigate', url: 'https://a.test/{{path}}' },
      { action: 'type', text: '{{email}}', el: { name: 'email', tag: 'input' } },
      { action: 'select_option', option: '{{plan}}', el: { name: 'plan', tag: 'select' } },
      { action: 'click', el: { text: 'Go', type: 'button' } },
      { action: 'press_key', key: 'Enter' },
      { action: 'scroll', direction: 'down', amount: 200 },
      { action: 'wait', selector: '#done', timeout: 5 },
      { action: 'handle_dialog', prompt_text: 'ok' },
    ];
    const result = await play(KEY, BROWSER, pb(steps, { defaults: { path: 'signup', plan: 'Pro' } }), { email: 'a@b' });
    assert.deepEqual(result, { steps: 8, total: 8, fellBack: false });
    assert.deepEqual(
      commands().map((c) => [c.action, c.params]),
      [
        ['navigate', { url: 'https://a.test/signup' }],
        ['type', { selector: '[data-ac-id="1"]', text: 'a@b' }],
        ['evaluate_raw', commands()[2].params],
        ['click', { selector: '[data-ac-id="2"]' }],
        ['press_key', { key: 'Enter' }],
        ['scroll', { direction: 'down', amount: 200 }],
        ['wait', { selector: '#done', timeout: 5 }],
        ['handle_dialog', { accept: true, prompt_text: 'ok' }],
      ],
    );
    assert.match(commands()[2].params.expression, /const want = "Pro"/);
    assert.equal(commands()[0].timeout, NAVIGATE_TIMEOUT_MS);
  });

  it('runs the checkpoint after navigations, clicks and key presses only', async () => {
    const checkpoint = mock.fn();
    const steps = [
      { action: 'navigate', url: 'https://a.test/' },
      { action: 'scroll', direction: 'down' },
      { action: 'click', el: { text: 'Go', type: 'button' } },
      { action: 'press_key', key: 'Tab' },
    ];
    await play(KEY, BROWSER, pb(steps), {}, { checkpoint });
    assert.equal(checkpoint.mock.callCount(), 3);
  });

  it('clicks the recorded element while a data-driven label keeps its default', async () => {
    const steps = [{ action: 'click', el: { text: '{{plan}}', type: 'button', domId: 'plan-basic' } }];
    await play(KEY, BROWSER, pb(steps, { defaults: { plan: 'Basic' } }));
    assert.deepEqual(commands()[0].params, { selector: '[data-ac-id="4"]' });
  });

  it('finds a data-driven click by its new value alone', async () => {
    const steps = [{ action: 'click', el: { text: '{{choice}}', type: 'link', domId: 'gone' } }];
    await play(KEY, BROWSER, pb(steps, { defaults: { choice: 'Basic' } }), { choice: 'go' });
    assert.deepEqual(commands()[0].params, { selector: '[data-ac-id="2"]' });
  });

  it('uploads the file() value passed for the step’s variable', async () => {
    const steps = [{ action: 'upload_file', file: '{{cv}}' }];
    await play(KEY, BROWSER, pb(steps), { cv: FILE });
    assert.match(commands()[0].params.expression, /"cv\.pdf"/);
  });

  describe('without autoHeal', () => {
    const noHeal = (steps, vars = {}) => play(KEY, BROWSER, pb(steps), vars, { autoHeal: false });

    it('fails with 422 naming the step that broke', async () => {
      await assert.rejects(noHeal([{ action: 'press_key', key: 'Tab' }, { action: 'teleport' }]), {
        status: 422,
        message: 'Step 2 of 2 (teleport) failed: unknown step teleport',
      });
    });

    it('treats an inherited property name as an unknown step', async () => {
      await assert.rejects(noHeal([{ action: 'constructor' }]), { message: /unknown step constructor/ });
    });

    it('fails with the browser’s error, or a generic one', async () => {
      answer = () => ({ ok: false, error: 'net::ERR' });
      await assert.rejects(noHeal([{ action: 'navigate', url: 'https://a.test/' }]), { message: /failed: net::ERR$/ });
      answer = () => ({ ok: false });
      await assert.rejects(noHeal([{ action: 'press_key', key: 'Tab' }]), { message: /failed: press_key failed$/ });
    });

    it('refuses an upload without a file() value', async () => {
      await assert.rejects(noHeal([{ action: 'upload_file', file: '{{cv}}' }], { cv: 'path.pdf' }), {
        message: /cv needs a file\(\) value/,
      });
      await assert.rejects(noHeal([{ action: 'upload_file' }]), { message: /this upload needs a file\(\) value/ });
    });

    it('reports an upload the page refused', async () => {
      answer = (a) =>
        a === 'evaluate_raw' ? { ok: true, data: { result: { ok: false, error: 'no file input' } } } : page(a);
      await assert.rejects(
        noHeal([{ action: 'upload_file', file: '{{cv}}', el: { name: 'email', tag: 'input' } }], { cv: FILE }),
        {
          message: /no file input/,
        },
      );
    });

    it('lists the options when a select finds no match', async () => {
      answer = (a) =>
        a === 'evaluate_raw'
          ? { ok: true, data: { result: { ok: false, error: 'no single option matches', options: ['A', 'B'] } } }
          : page(a);
      await assert.rejects(noHeal([{ action: 'select_option', option: 'Z', el: { name: 'plan', tag: 'select' } }]), {
        message: /no single option matches \(options: A \| B\)$/,
      });
      answer = (a) =>
        a === 'evaluate_raw' ? { ok: true, data: { result: { ok: false, error: 'dropdown not found' } } } : page(a);
      await assert.rejects(noHeal([{ action: 'select_option', option: 'Z', el: { name: 'plan', tag: 'select' } }]), {
        message: /dropdown not found$/,
      });
    });

    it('re-analyzes for a few seconds before giving up on an element', async () => {
      mock.timers.enable({ apis: ['setTimeout'] });
      const failed = assert.rejects(noHeal([{ action: 'click', el: { text: 'Missing', type: 'button' } }]), {
        message: /no element matching "Missing"/,
      });
      await advance(FIND_RETRY_MS, FIND_ATTEMPTS + 2);
      await failed;
      assert.equal(browser.calls.filter((c) => c.action === 'analyze').length, FIND_ATTEMPTS);
    });
  });

  describe('healing', () => {
    const broken = () =>
      pb([{ action: 'press_key', key: 'Tab' }, { action: 'teleport' }], { secrets: ['pw'], labels: ['go'] });

    it('lets the agent finish from the broken step and saves its steps as a draft', async () => {
      process.env.OPENAI_API_KEY = 'sk-host';
      const llm = stubLlm([textReply('DONE: finished')]);
      const result = await play(KEY, BROWSER, broken(), { email: 'ada@x.test', pw: 'hunter2', go: 'Go now' });
      assert.deepEqual(result, {
        steps: 1,
        total: 2,
        fellBack: true,
        healed: true,
        draft: 'demo:draft',
        text: 'DONE: finished',
      });
      const draft = keyConfig.getPlaybook(KEY, 'demo:draft');
      assert.equal(draft.healedFrom, 1);
      assert.deepEqual(draft.steps, [{ action: 'press_key', key: 'Tab' }]);
      const [system, user] = llm.requests[0].messages;
      assert.match(user.content, /already did 1 of 2 steps .*\(unknown step teleport\)/);
      assert.match(system.content, /\{\{email\}\} = "ada@x\.test"/);
      assert.doesNotMatch(system.content, /Go now|hunter2/, 'labels are not data, secrets are never shown');
    });

    it('hands over to a person when the agent says it failed', async () => {
      process.env.OPENAI_API_KEY = 'sk-host';
      stubLlm([textReply('Tried.\nFAILED: the site is down')]);
      const requestHuman = mock.fn(async () => 'done');
      const result = await play(KEY, BROWSER, broken(), {}, { requestHuman });
      assert.deepEqual(result, { steps: 1, total: 2, fellBack: true, healed: false, text: 'Finished by a person.' });
      const [attention] = requestHuman.mock.calls[0].arguments;
      assert.equal(attention.reason, 'heal_failed');
      assert.match(attention.message, /Replay broke at step 2 .*the site is down/s);
    });

    it('throws the agent’s failure when no person is reachable', async () => {
      process.env.OPENAI_API_KEY = 'sk-host';
      stubLlm([textReply('FAILED: blocked')]);
      await assert.rejects(play(KEY, BROWSER, broken()), { message: 'FAILED: blocked' });
    });

    it('fails when the agent runs out of steps', async () => {
      process.env.OPENAI_API_KEY = 'sk-host';
      process.env.CHAT_MAX_ITERATIONS = '1';
      stubLlm([textReply('')]);
      await assert.rejects(play(KEY, BROWSER, broken()), { message: 'the agent hit its step limit' });
    });
  });

  describe('version-2 workflows', () => {
    const workflow = (steps) => ({ schemaVersion: 2, name: 'wf', steps });
    const NAV = { action: 'navigate', url: 'https://a.test/' };

    it('runs the whole draft in the browser and reports its assertions', async () => {
      answer = () => ({ ok: true, data: { status: 'succeeded', assertions: 2, id: 'wr_1' } });
      const result = await play(
        KEY,
        BROWSER,
        workflow([NAV, { ...NAV, enabled: false }]),
        { q: 'x' },
        { autoHeal: false },
      );
      assert.deepEqual(result, { steps: 1, total: 2, fellBack: false, assertions: 2, runId: 'wr_1' });
      const [call] = browser.calls;
      assert.equal(call.action, 'workflow');
      assert.deepEqual([call.params.variables, call.params.autoHeal], [{ q: 'x' }, false]);
    });

    it('answers 422 when the browser’s run did not succeed', async () => {
      answer = () => ({ ok: true, data: { status: 'failed', error: 'assertion failed' } });
      await assert.rejects(play(KEY, BROWSER, workflow([NAV])), { status: 422, message: 'assertion failed' });
      answer = () => ({ ok: true, data: { status: 'cancelled' } });
      await assert.rejects(play(KEY, BROWSER, workflow([NAV])), { message: 'Playwright validation cancelled' });
    });

    it('refuses a workflow with a human checkpoint, which needs Oya Browser', async () => {
      await assert.rejects(play(KEY, BROWSER, workflow([NAV, { action: 'checkpoint' }])), { status: 409 });
    });

    it('refuses an invalid workflow with 400', async () => {
      await assert.rejects(play(KEY, BROWSER, workflow([{ action: 'navigate', url: 'ftp://x' }])), { status: 400 });
    });
  });
});

describe('play across tabs', () => {
  beforeEach(() => {
    answer = page;
    browser = scriptedBrowser(BROWSER, KEY, (a, p) => answer(a, p));
  });
  afterEach(() => {
    browser.disconnect();
    mock.restoreAll();
    mock.timers.reset();
    keyConfig.reset();
    delete process.env.OPENAI_API_KEY;
  });

  /** Replay without healing, so a tab that never arrives surfaces as itself. */
  const noHeal = (steps, vars = {}) => play(KEY, BROWSER, pb(steps), vars, { autoHeal: false });

  /** A tab list where the handoff tab carries a fresh SSO token, as a real one does. */
  const tabsWith = (urls) => (action, params) => {
    if (action === 'list_tabs')
      return { ok: true, data: { tabs: urls.map((url, i) => ({ id: i + 1, url, active: i === 0 })) } };
    return page(action, params);
  };

  it('finds the handoff tab by where it went, not by the token in its url', async () => {
    answer = tabsWith(['https://portal.example.com/web/auth', 'https://vendor.example.com/order/new?ssoToken=fresh-2']);
    const steps = [{ action: 'switch_tab', tabUrl: 'https://vendor.example.com/order/new?ssoToken=stale-1' }];
    await play(KEY, BROWSER, pb(steps), {});
    const switched = commands().find((c) => c.action === 'switch_tab');
    assert.deepEqual(switched.params, { tab_id: 2 });
  });

  it('waits for a tab the handoff has not opened yet', async () => {
    let listed = 0;
    answer = (action, params) => {
      if (action === 'list_tabs') {
        listed += 1;
        const urls =
          listed > 2
            ? ['https://portal.example.com/x', 'https://vendor.example.com/order/new']
            : ['https://portal.example.com/x'];
        return { ok: true, data: { tabs: urls.map((url, i) => ({ id: i + 1, url, active: i === 0 })) } };
      }
      return page(action, params);
    };
    mock.timers.enable({ apis: ['setTimeout'] });
    const run = noHeal([{ action: 'switch_tab', tabUrl: 'https://vendor.example.com/order/new' }]);
    await advance(FIND_RETRY_MS, FIND_ATTEMPTS + 2);
    await run;
    assert.ok(listed > 2, 'kept looking while the tab was opening');
    assert.ok(commands().some((c) => c.action === 'switch_tab'));
  });

  it('says which tab never arrived instead of carrying on in the wrong one', async () => {
    answer = tabsWith(['https://portal.example.com/web/auth']);
    mock.timers.enable({ apis: ['setTimeout'] });
    const failed = assert.rejects(noHeal([{ action: 'switch_tab', tabUrl: 'https://vendor.example.com/order/new' }]), {
      message: /no tab at https:\/\/vendor.example.com\/order\/new/,
    });
    await advance(FIND_RETRY_MS, FIND_ATTEMPTS + 2);
    await failed;
  });

  it('types into the focused field, filling the value the run was given', async () => {
    await play(KEY, BROWSER, pb([{ action: 'keyboard_type', text: '{{member_id}}' }]), { member_id: 'MEM000000001' });
    const typed = commands().find((c) => c.action === 'keyboard_type');
    assert.deepEqual(typed.params, { text: 'MEM000000001' });
  });

  it('treats a tab that is already closed as closed', async () => {
    answer = tabsWith(['https://portal.example.com/web/auth']);
    mock.timers.enable({ apis: ['setTimeout'] });
    const run = noHeal([{ action: 'close_tab', tabUrl: 'https://vendor.example.com/done' }]);
    await advance(FIND_RETRY_MS, FIND_ATTEMPTS + 2);
    await run;
    assert.ok(!commands().some((c) => c.action === 'close_tab'));
  });
  describe('play without re-reading the page', () => {
    beforeEach(() => {
      answer = page;
      browser = scriptedBrowser(BROWSER, KEY, (a, p) => answer(a, p));
    });
    afterEach(() => {
      browser.disconnect();
      mock.restoreAll();
      mock.timers.reset();
      keyConfig.reset();
      delete process.env.OPENAI_API_KEY;
    });

    /** How many times the page was serialized. */
    const analyses = () => browser.calls.filter((c) => c.action === 'analyze').length;

    it('analyzes once for several steps on the same page', async () => {
      const steps = [
        { action: 'type', text: 'a@b', el: { name: 'email', tag: 'input' } },
        { action: 'select_option', option: 'Pro', el: { name: 'plan', tag: 'select' } },
        { action: 'type', text: 'c@d', el: { name: 'email', tag: 'input' } },
      ];
      await play(KEY, BROWSER, pb(steps), {});
      assert.equal(analyses(), 1, 'the page was read once, not once per step');
    });

    it('reads the page again after a step that could have changed it', async () => {
      const steps = [
        { action: 'type', text: 'a@b', el: { name: 'email', tag: 'input' } },
        { action: 'press_key', key: 'Enter' },
        { action: 'type', text: 'c@d', el: { name: 'email', tag: 'input' } },
      ];
      await play(KEY, BROWSER, pb(steps), {});
      assert.equal(analyses(), 2, 'once before the key, once after');
    });

    it('clicks a recorded test id without reading the page at all', async () => {
      const steps = [{ action: 'click', el: { tag: 'button', testId: 'next-page', text: 'Next' } }];
      await play(KEY, BROWSER, pb(steps), {});
      assert.equal(analyses(), 0);
      const clicked = commands().find((c) => c.action === 'click');
      assert.deepEqual(clicked.params, { selector: '[data-testid="next-page"]' });
    });

    it('clicks a link by its own target without reading the page', async () => {
      const steps = [{ action: 'click', el: { tag: 'a', href: '/wiki/Managed_care', text: 'Managed care' } }];
      await play(KEY, BROWSER, pb(steps), {});
      assert.equal(analyses(), 0);
      assert.deepEqual(commands().find((c) => c.action === 'click').params, {
        selector: 'a[href="/wiki/Managed_care"]',
      });
    });

    it('falls back to matching when the fast path misses', async () => {
      const byId: any[] = [{ id: 9, type: 'button', tag: 'button', text: 'Go', testId: 'slow', visible: true }];
      let matched = 0;
      answer = (action, params) => {
        // The selector no longer resolves, though the element is still on the page:
        // a framework re-rendered it, or it moved into a frame.
        if (action === 'click' && params.selector === '[data-testid="slow"]') return { ok: false, error: 'not found' };
        if (action === 'click' && params.selector === '[data-ac-id="9"]') matched += 1;
        if (action === 'analyze') return { ok: true, data: { elements: byId } };
        return page(action, params);
      };
      await play(KEY, BROWSER, pb([{ action: 'click', el: { tag: 'button', testId: 'slow', text: 'Go' } }]), {});
      assert.equal(matched, 1, 'it still clicked, by matching the analysis');
      assert.ok(analyses() > 0);
    });

    it('does not take the fast path for a click whose value comes from a variable', async () => {
      const steps = [{ action: 'click', el: { tag: 'button', testId: 'plan', text: '{{plan}}' } }];
      await play(KEY, BROWSER, pb(steps, { defaults: { plan: 'Basic' } }), { plan: 'Pro' });
      assert.ok(analyses() > 0, 'a data-driven click aims by value, so it must read the page');
    });
  });

  describe('play at a human pace', () => {
    beforeEach(() => {
      answer = page;
      browser = scriptedBrowser(BROWSER, KEY, (a, p) => answer(a, p));
    });
    afterEach(() => {
      browser.disconnect();
      mock.restoreAll();
      mock.timers.reset();
      keyConfig.reset();
      delete process.env.OPENAI_API_KEY;
    });

    it('waits between steps rather than firing them as fast as the network allows', async () => {
      const started = Date.now();
      const steps = [
        { action: 'type', text: 'a@b', el: { name: 'email', tag: 'input' } },
        { action: 'type', text: 'c@d', el: { name: 'email', tag: 'input' } },
      ];
      await play(KEY, BROWSER, pb(steps), {});
      // Two steps, so at least one pause at the shortest it can be.
      assert.ok(Date.now() - started >= REPLAY_PAUSE_MS.min, 'a replay that beats a human gets caught');
    });

    it('waits longer once the page has changed under it, the way a person reads it', async () => {
      const started = Date.now();
      await play(KEY, BROWSER, pb([{ action: 'press_key', key: 'Enter' }]), {});
      assert.ok(Date.now() - started >= REPLAY_SETTLE_MS.min);
    });

    it('stops when the fast selector turns out to point at something else', async () => {
      answer = (action, params) => {
        // The test id now belongs to a different control.
        if (action === 'click' && params.selector === '[data-testid="submit"]')
          return {
            ok: true,
            data: { clicked: true, handle: { tag: 'button', testId: 'submit', text: 'Delete account' } },
          };
        return page(action, params);
      };
      const steps = [{ action: 'click', el: { tag: 'button', testId: 'submit', text: 'Submit claim' } }];
      await assert.rejects(play(KEY, BROWSER, pb(steps), {}, { autoHeal: false }), /no longer points at/);
    });

    it('accepts a click whose text merely grew a suffix', async () => {
      answer = (action, params) => {
        if (action === 'click' && params.selector === '[data-testid="next"]')
          return { ok: true, data: { clicked: true, handle: { tag: 'button', testId: 'next', text: 'Next page' } } };
        return page(action, params);
      };
      const steps = [{ action: 'click', el: { tag: 'button', testId: 'next', text: 'Next' } }];
      const result = await play(KEY, BROWSER, pb(steps), {});
      assert.equal(result.steps, 1);
    });
  });

  describe('play fills the placeholders in a handle', () => {
    beforeEach(() => {
      answer = page;
      browser = scriptedBrowser(BROWSER, KEY, (a, p) => answer(a, p));
    });
    afterEach(() => {
      browser.disconnect();
      mock.restoreAll();
      mock.timers.reset();
      keyConfig.reset();
      delete process.env.OPENAI_API_KEY;
    });

    it("clicks a link whose target was recorded with the run's own value in it", async () => {
      // Reddit: the post link is recorded as "{{start}}comments/abc/", because the
      // listing url was a variable. Unfilled, that handle matches nothing at all.
      const steps = [{ action: 'click', el: { tag: 'a', rawHref: '{{start}}comments/abc/' } }];
      await play(KEY, BROWSER, pb(steps), { start: 'https://r.test/r/x/' });
      const clicked = commands().find((c) => c.action === 'click');
      assert.deepEqual(clicked.params, { selector: 'a[href="https://r.test/r/x/comments/abc/"]' });
    });

    it('matches a field whose name carried a value', async () => {
      const elements = [{ id: 9, type: 'input', tag: 'input', name: 'field-Basic', visible: true }];
      answer = (action, params) => (action === 'analyze' ? { ok: true, data: { elements } } : page(action, params));
      const steps = [{ action: 'type', text: 'x', el: { tag: 'input', type: 'input', name: 'field-{{plan}}' } }];
      await play(KEY, BROWSER, pb(steps), { plan: 'Basic' });
      const typed = commands().find((c) => c.action === 'type');
      assert.deepEqual(typed.params, { selector: '[data-ac-id="9"]', text: 'x' });
    });
  });
});
