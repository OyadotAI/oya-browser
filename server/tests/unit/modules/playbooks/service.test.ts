/**
 * Unit tests for the playbooks facade: what it exports, and that a replay is
 * counted as a product event with how it ended, whoever asked for it.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const playbooks = await import('../../../../src/modules/playbooks/service.ts');
const analytics = await import('../../../../src/platform/analytics.ts');
const { scriptedBrowser } = await import('../../support/agent.ts');
const { stubFetch, json } = await import('../../support/http.ts');

const KEY = 'facade-key';
const BROWSER = 'b-facade';

/** The playbook_replayed events PostHog was sent, by outcome. */
async function replayedEvents(calls) {
  await analytics.drain();
  return calls
    .filter((c) => c.url.includes('/batch/'))
    .flatMap((c) => JSON.parse(c.init.body).batch)
    .filter((e) => e.event === 'playbook_replayed')
    .map((e) => [e.properties.outcome, e.properties.healed]);
}

describe('playbooks facade', () => {
  it('exports matching, checking, variables, the export, the catalog and replay', () => {
    assert.deepEqual(Object.keys(playbooks).sort(), [
      'create',
      'list',
      'matchElement',
      'missingVariables',
      'outcomeOf',
      'play',
      'promote',
      'remove',
      'rename',
      'renderPlaywright',
      'sanitizeSteps',
      'templateValues',
      'validateWorkflow',
      'variablesOf',
    ]);
  });
});

describe('play, counted', () => {
  let browser;
  beforeEach(() => {
    process.env.POSTHOG_KEY = 'phc_test';
    process.env.POSTHOG_HOST = 'https://ph.example.test';
    analytics.resetForTests();
    browser = scriptedBrowser(BROWSER, KEY, () => ({ ok: true, data: {} }));
  });
  afterEach(() => {
    browser.disconnect();
    mock.restoreAll();
    delete process.env.POSTHOG_KEY;
    delete process.env.POSTHOG_HOST;
  });

  it('counts a run whose steps all ran as ok, and a run that threw as failed', async () => {
    const calls = stubFetch(() => json({}));
    const pb = { name: 'demo', prompt: 'x', steps: [{ action: 'press_key', key: 'Tab' }], defaults: {} };
    await playbooks.play(KEY, BROWSER, pb, {}, { autoHeal: false });
    await assert.rejects(
      playbooks.play(KEY, BROWSER, { ...pb, steps: [{ action: 'teleport' }] }, {}, { autoHeal: false }),
    );
    await new Promise((r) => setTimeout(r, 5));
    // Each event waits on its own owner lookup, so they may be sent in either order.
    assert.deepEqual((await replayedEvents(calls)).sort(), [
      ['failed', false],
      ['ok', false],
    ]);
  });

  it('tells a run the agent healed from one a person finished, from what the replay answered', () => {
    assert.equal(playbooks.outcomeOf({ fellBack: true, healed: true }, false), 'healed');
    assert.equal(playbooks.outcomeOf({ fellBack: true, healed: false }, false), 'handed_over');
    assert.equal(playbooks.outcomeOf({ fellBack: false }, false), 'ok');
    assert.equal(playbooks.outcomeOf(undefined, true), 'failed');
  });
});
