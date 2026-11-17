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
const { getConnection } = await import('../../../../src/platform/storage/index.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');
const { fingerprint } = await import('../../../../src/platform/audit.ts');

const KEY = 'facade-key';
const BROWSER = 'b-facade';
const OWNER = fingerprint(KEY);

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
      'EXPORT_FORMAT',
      'EXPORT_VERSION',
      'answerField',
      'create',
      'exportPlaybook',
      'importPlaybook',
      'lastRuns',
      'list',
      'matchElement',
      'missingVariables',
      'outcomeOf',
      'play',
      'promote',
      'remove',
      'rename',
      'renderPlaywright',
      'runsOf',
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

describe('play, remembered', () => {
  const pb = { name: 'kept', steps: [{ action: 'press_key', key: 'Tab' }], defaults: {} };
  let browser;
  beforeEach(async () => {
    browser = scriptedBrowser(BROWSER, KEY, () => ({ ok: true, data: {} }));
    await getConnection().delete('playbook_runs', {});
    await keyConfig.savePlaybook(KEY, 'kept', pb);
  });
  afterEach(() => {
    browser.disconnect();
    mock.restoreAll();
  });
  /** The runs the history holds for `name` once the background write has landed. */
  const runsFor = async (name: string) => (await new Promise((r) => setTimeout(r, 20)), playbooks.runsOf(OWNER, name));

  it('keeps a finished run and a failed one in the playbook’s history, newest first', async () => {
    await playbooks.play(KEY, BROWSER, pb, {}, { autoHeal: false });
    await runsFor('kept');
    await assert.rejects(
      playbooks.play(KEY, BROWSER, { ...pb, steps: [{ action: 'teleport' }] }, {}, { autoHeal: false }),
    );
    const runs = await runsFor('kept');
    assert.deepEqual(
      runs.map((r) => [r.status, r.browserId]),
      [
        ['failed', BROWSER],
        ['succeeded', BROWSER],
      ],
    );
    assert.match(runs[0].error ?? '', /teleport/);
  });

  it('blanks a secret the run was given if its error quotes it', async () => {
    const secretPb = { ...pb, secrets: ['pw'], steps: [{ action: 'teleport', text: '{{pw}}' }] };
    await keyConfig.savePlaybook(KEY, 'kept', secretPb);
    const quoting = { ...secretPb, steps: [{ action: 'teleport-hunter2' }] };
    await assert.rejects(playbooks.play(KEY, BROWSER, quoting, { pw: 'hunter2' }, { autoHeal: false }));
    const [failed] = await runsFor('kept');
    assert.doesNotMatch(failed.error ?? '', /hunter2/);
    assert.match(failed.error ?? '', /\[secret\]/);
  });

  it('leaves out a run whose playbook was deleted while it ran, so a new one with that name starts clean', async () => {
    await keyConfig.deletePlaybook(KEY, 'kept');
    await playbooks.play(KEY, BROWSER, pb, {}, { autoHeal: false });
    assert.deepEqual(await runsFor('kept'), []);
  });

  it('leaves the editor’s unsaved test runs out of any history', async () => {
    await playbooks.play(KEY, BROWSER, { ...pb, name: 'draft', transient: true }, {}, { autoHeal: false });
    assert.deepEqual(await runsFor('draft'), []);
  });

  it('a history the database refuses never fails the run', async () => {
    const warn = mock.method(console, 'warn', () => {});
    const db = getConnection();
    const real = db.select.bind(db);
    mock.method(db, 'select', async (table: string, ...rest) => {
      if (table === 'playbook_runs') throw new Error('db down');
      return real(table, ...rest);
    });
    assert.deepEqual(await playbooks.play(KEY, BROWSER, pb, {}, { autoHeal: false }), {
      steps: 1,
      total: 1,
      fellBack: false,
    });
    await new Promise((r) => setTimeout(r, 20));
    assert.match(String(warn.mock.calls[0]?.arguments[0]), /run history not saved: db down/);
  });
});
