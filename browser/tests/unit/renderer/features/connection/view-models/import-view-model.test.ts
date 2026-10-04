/**
 * Unit tests for "Imported logins" on the account page: pick one of the
 * browsers on this computer, import it, and read plainly how it went; the
 * latest import and the ones before it.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  earlierImports,
  emptyImportText,
  importAmount,
  importBlocked,
  importStatus,
  noSources,
  sourceLabel,
} from '../../../../../../src/renderer/features/connection/view-models/import-view-model.ts';
import { ago } from '../../../../../../src/renderer/ui/ago.ts';
import { build, settle } from '../harness.ts';

const SOURCES = [
  { id: 'firefox', name: 'Firefox', isDefault: true },
  { id: 'chrome', name: 'Chrome', isDefault: false },
];
const DAY = 86_400_000;

/** The feature, connected, with `sources` installed. */
async function connected(sources: unknown[] = SOURCES) {
  const app = build({ importSources: sources });
  await settle();
  app.fake.emit('onWsStatus', { connected: true, profileName: 'Default' });
  return app;
}

/** Whether Import can be pressed. */
const canImport = (app: Awaited<ReturnType<typeof connected>>) =>
  !app.imports.state.running && !importBlocked(app.imports.state);

/** The detail line under the latest import. */
function detail(app: Awaited<ReturnType<typeof connected>>): string {
  const latest = app.imports.state.history[0];
  return `${ago(latest.at)} · ${importAmount(latest)}`;
}

describe('Import logins', () => {
  it("offers the browsers on this computer, the person's default first", async () => {
    const app = await connected();
    assert.deepEqual(
      (app.imports.state.sources ?? []).map((s) => [s.id, sourceLabel(s)]),
      [
        ['firefox', 'Firefox (your default browser)'],
        ['chrome', 'Chrome'],
      ],
    );
    assert.equal(app.imports.state.chosen, 'firefox');
    assert.equal(canImport(app), true);
  });

  it('imports the chosen browser and says what is happening, then what came over', async () => {
    const app = await connected();
    app.imports.choose('chrome');
    await app.imports.start();
    assert.deepEqual(app.fake.called('reimportBrowser'), [['chrome']]);
    app.fake.emit('onMirrorStatus', { started: true });
    assert.equal(importStatus(app.imports.state).text, 'Reading your logins from Chrome…');
    assert.equal(canImport(app), false);
    app.fake.emit('onMirrorStatus', { done: true, source: 'Chrome', profiles: 2, cookies: 412 });
    assert.equal(importStatus(app.imports.state).text, 'Done. You are signed in to those sites here now.');
    assert.equal(detail(app), 'just now · 412 cookies');
    assert.equal(canImport(app), true);
  });

  it('still says what was imported after the reconnect that follows an import', async () => {
    const app = await connected();
    app.fake.emit('onMirrorStatus', { done: true, source: 'Firefox', profiles: 1, cookies: 2 });
    app.fake.emit('onWsStatus', { connected: false });
    assert.equal(importStatus(app.imports.state).text, 'Connect to Oya to import your logins.');
    app.fake.emit('onWsStatus', { connected: true });
    assert.equal(importStatus(app.imports.state).text, 'Done. You are signed in to those sites here now.');
    assert.equal(canImport(app), true);
  });

  it('says why an import failed, and lets it be tried again', async () => {
    const app = await connected();
    await app.imports.start();
    app.fake.emit('onMirrorStatus', { done: true, error: 'Chrome never opened its debugging port' });
    assert.deepEqual(importStatus(app.imports.state), {
      text: 'Chrome never opened its debugging port',
      kind: 'error',
    });
    assert.equal(canImport(app), true);
  });

  it('says why an import could not start', async () => {
    const app = build({ importSources: SOURCES, reimportBrowser: () => Promise.reject(new Error('busy')) });
    await settle();
    app.fake.emit('onWsStatus', { connected: true });
    await app.imports.start();
    assert.deepEqual(importStatus(app.imports.state), { text: 'busy', kind: 'error' });
  });

  it('says so when the browser had nothing to bring over', async () => {
    const app = await connected();
    app.fake.emit('onMirrorStatus', { done: true, empty: true });
    assert.equal(importStatus(app.imports.state).text, 'Nothing to import: that browser has no profiles with logins.');
  });

  it('knows it is connected when the app connected before this page was listening', async () => {
    const app = build({ importSources: SOURCES, getStatus: { connected: true, browserId: 'b1', profileName: 'Work' } });
    await settle();
    assert.equal(canImport(app), true);
    assert.equal(importStatus(app.imports.state).text, '');
  });

  it('waits for a connection, and says so', async () => {
    const app = build({ importSources: SOURCES });
    await settle();
    app.fake.emit('onWsStatus', { connected: false });
    assert.equal(canImport(app), false);
    assert.equal(importStatus(app.imports.state).text, 'Connect to Oya to import your logins.');
  });

  it('shows the latest import from the saved history, with when and how much, and the ones before it', async () => {
    const imports = [
      { source: 'Chrome', at: Date.now() - 3 * DAY, sites: 412, cookies: 3000, profiles: 2 },
      { source: 'Firefox', at: Date.now() - 10 * DAY, cookies: 1, profiles: 1 },
    ];
    const app = build({ importSources: SOURCES, getConfig: { serverUrl: '', imports } });
    await settle();
    assert.equal(app.imports.state.history[0].source, 'Chrome');
    assert.equal(detail(app), '3 days ago · 412 sites');
    const earlier = earlierImports(app.imports.state).map((r) => `${r.source} · ${ago(r.at)} · ${importAmount(r)}`);
    assert.deepEqual(earlier, ['Firefox · last week · 1 cookie']);
  });

  it('puts a finished import at the top as it lands', async () => {
    const app = await connected();
    app.fake.emit('onMirrorStatus', {
      done: true,
      source: 'Chrome',
      profiles: 1,
      cookies: 9,
      sites: 4,
      at: Date.now(),
    });
    assert.equal(app.imports.state.history[0].source, 'Chrome');
    assert.equal(detail(app), 'just now · 4 sites');
    assert.deepEqual(earlierImports(app.imports.state), []);
  });

  it('with no import yet, says what one is for and names the browsers this computer has', async () => {
    const app = await connected();
    assert.equal(app.imports.state.history.length, 0);
    assert.equal(
      emptyImportText(app.imports.state.sources),
      'Bring your logins from Firefox or Chrome, so you are already signed in here.',
    );
    assert.equal(
      emptyImportText(null),
      'Bring your logins from Chrome, Arc or Firefox, so you are already signed in here.',
    );
  });

  it('says so when no supported browser is installed', async () => {
    const app = await connected([]);
    assert.equal(noSources(app.imports.state), true);
    assert.equal(importStatus(app.imports.state).text, 'No supported browser found on this computer.');
  });

  it('treats a failed listing as no browsers', async () => {
    const app = build({ importSources: () => Promise.reject(new Error('denied')) });
    await settle();
    assert.equal(noSources(app.imports.state), true);
  });
});
