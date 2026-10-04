/**
 * Unit tests for the Inspect Source pane: errors and notices are shown where
 * the read would be, the page is named and marked stale, an inspected element
 * never shows beside another read's HTML, the newest format switch wins,
 * Refresh waits for its answer, the default format comes from settings, and
 * Clear brings the prompt back.
 */
import { describe, it, mock, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import {
  SourceViewModel,
  sourceLook,
} from '../../../../../../src/renderer/features/inspect/view-models/source-view-model.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';
import { fakeBridge } from '../../../support/bridge.ts';

/** Lets pending promises settle. */
const settle = () => new Promise((resolve) => setImmediate(resolve));

/** A Source pane over a fake bridge answering `answers`. */
async function pane(answers: Record<string, unknown> = {}, clipboard = { writeText: async (_t: string) => {} }) {
  const fake = fakeBridge({ getUiPreferences: {}, ...answers });
  const clears = new Map<string, () => void>();
  const shown: string[] = [];
  const panel = {
    onClear: (p: string, fn: () => void) => (clears.set(p, fn), () => {}),
    show: (p: string) => void shown.push(p),
  };
  const vm = new SourceViewModel({ bridge: fake.bridge, panel: panel as never, clipboard });
  await settle();
  return { fake, clears, shown, vm, look: () => sourceLook(vm.state) };
}

afterEach(() => mock.timers.reset());

describe('SourceViewModel', () => {
  it('prompts for a read before there is one', async () => {
    const { look } = await pane();
    assert.deepEqual(
      [look().url, look().page.text, look().html.text, look().page.empty],
      [
        'The current page, as the agent reads it. Refresh to read it.',
        'Refresh, or right-click the page and choose View Page Source.',
        'No HTML yet.',
        true,
      ],
    );
  });

  it('shows why the page could not be read, instead of saying there is nothing', async () => {
    const { vm, look } = await pane({ getPageSource: { html: '', markdown: '', error: 'Page is gone' } });
    await vm.refresh();
    assert.deepEqual([look().page.text, look().page.error], ['Could not read the page: Page is gone', true]);
  });

  it('shows a failed call as an error where the read would be', async () => {
    const { vm, look } = await pane({
      getPageSource: () => {
        throw new Error('no tab');
      },
    });
    await vm.refresh();
    assert.equal(look().page.text, 'Could not read the page: no tab');
  });

  it('shows the agent notice when the page cannot be read while it drives', async () => {
    const notice = 'The agent is using this page. Take control to read it.';
    const { vm, look } = await pane({ getPageSource: { html: '<p>', markdown: '', notice, url: 'https://a.test/' } });
    await vm.refresh();
    assert.equal(look().page.text, notice);
  });

  it('names the page it read, and says when the page changed', async () => {
    const { fake, vm, look } = await pane({ getPageSource: { html: '<p>', markdown: '# A', url: 'https://a.test/' } });
    await vm.refresh();
    assert.equal(look().url, 'Read from https://a.test/');
    fake.emit('onUrlChanged', 'https://b.test/');
    assert.deepEqual([look().url, vm.state.stale], ['The page changed. Refresh to read it again.', true]);
  });

  it('says nothing of a page change before a page was read', async () => {
    const { fake, vm } = await pane();
    fake.emit('onUrlChanged', 'https://b.test/');
    assert.equal(vm.state.stale, false);
  });

  it('does not show an inspected element beside the HTML of another read', async () => {
    const { fake, vm, look, shown } = await pane({
      getPageSource: { html: '<p>old</p>', markdown: '# A', url: 'https://a.test/' },
    });
    await vm.refresh();
    fake.emit('onInspectResult', { ok: true, data: { page: '# Button', blocks: [] } });
    await settle();
    assert.equal(look().page.text, '# Button');
    assert.doesNotMatch(look().html.text, /old/);
    assert.deepEqual(shown, ['source']);
  });

  it('shows why an inspect failed', async () => {
    const { vm, look } = await pane();
    await vm.inspected({ ok: false, error: 'Element gone' });
    assert.equal(look().page.text, 'Could not read the page: Element gone');
  });

  it('opens on View Page Source with the page it was sent', async () => {
    const { fake, look, shown } = await pane();
    fake.emit('onViewSource', { html: '<p>x</p>', markdown: '# X', url: 'https://x.test/' });
    await settle();
    assert.deepEqual([shown, look().html.text, look().page.text], [['source'], '<p>x</p>', '# X']);
  });

  it('renders the kept analysis again in a picked format', async () => {
    const analysis = { format: 'markdown', blocks: [] };
    const { fake, vm, look } = await pane({
      getPageSource: { html: '', markdown: '# A', analysis, url: 'https://a.test/' },
      renderPage: (_a: unknown, format: string) => `as ${format}`,
    });
    await vm.refresh();
    await vm.pick('toon');
    assert.deepEqual([look().page.text, look().label], ['as toon', 'TOON']);
    await vm.pick('toon');
    assert.equal(fake.called('renderPage').length, 1, 'no second render for the format it is in');
  });

  it('says why a format could not be shown', async () => {
    const { vm, look } = await pane({
      getPageSource: { html: '', markdown: '# A', analysis: { format: 'markdown' }, url: 'u' },
      renderPage: () => {
        throw new Error('bad');
      },
    });
    await vm.refresh();
    await vm.pick('jsonl');
    assert.equal(look().page.text, 'Could not show this format: bad');
  });

  it('lets the newest format switch win over a slower earlier one', async () => {
    const pending: (() => void)[] = [];
    const { vm, look } = await pane({
      getPageSource: {
        html: '',
        markdown: '# A',
        analysis: { format: 'markdown', blocks: [] },
        url: 'https://a.test/',
      },
      renderPage: (_a: unknown, format: string) =>
        new Promise((resolve) => pending.push(() => resolve('as ' + format))),
    });
    await vm.refresh();
    const toon = vm.pick('toon');
    const jsonl = vm.pick('jsonl');
    pending[1]();
    await jsonl;
    pending[0]();
    await toon;
    assert.equal(look().page.text, 'as jsonl');
  });

  it('keeps Refresh off while it reads', async () => {
    let release: (value: unknown) => void = () => {};
    const { vm } = await pane({ getPageSource: () => new Promise((resolve) => (release = resolve)) });
    const reading = vm.refresh();
    assert.equal(vm.state.refreshing, true);
    release({ html: '', markdown: '' });
    await reading;
    assert.equal(vm.state.refreshing, false);
  });

  it('starts at the default format from settings, ignoring one it does not know', async () => {
    const { vm } = await pane({ getUiPreferences: { pageFormat: 'jsonl' } });
    assert.equal(vm.state.format, 'jsonl');
    vm.setDefault('yaml');
    assert.equal(vm.state.format, 'jsonl');
  });

  it('saves a default format chosen in settings, and shows the read in it', async () => {
    const { fake, vm } = await pane();
    await vm.chooseDefault('toon');
    assert.deepEqual(fake.called('saveUiPreferences'), [[{ pageFormat: 'toon' }]]);
    assert.equal(vm.state.format, 'toon');
  });

  it('copies a half and says so for a moment', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const copied: string[] = [];
    const { vm } = await pane(
      { getPageSource: { html: '<p>', markdown: '# A', url: 'u' } },
      {
        writeText: async (t: string) => void copied.push(t),
      },
    );
    await vm.refresh();
    await vm.copy('html');
    assert.deepEqual([copied, vm.state.copyLabels.html], [['<p>'], 'Copied']);
    mock.timers.tick(C.COPIED_MS);
    assert.equal(vm.state.copyLabels.html, 'Copy');
  });

  it('forgets the read on the pane’s Clear', async () => {
    const { clears, vm, look } = await pane({ getPageSource: { html: '<p>', markdown: '# A', url: 'u' } });
    await vm.refresh();
    clears.get('source')?.();
    assert.deepEqual([vm.state.read, look().html.text], [null, 'No HTML yet.']);
  });
});
