/**
 * Unit tests for the console's actions: stopping browsers, screenshots, a
 * persona's browsers, the Slack return, and pruning the selection.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import {
  browsersOf,
  handleSlackReturn,
  pruneSelection,
  stopBrowsers,
  stopToast,
  takeScreenshot,
} from '@/app/dashboard/_console/actions';
import { INITIAL_VIEW, NO_FILTER } from '@/app/dashboard/_console/view';
import type { BrowserRow, Persona } from '@/components/dashboard/types';
import { fakeFetch } from '../../../support';

afterEach(() => {
  vi.unstubAllGlobals();
  history.replaceState(null, '', '/');
});

/** Rows with these ids. */
const rows = (...ids: string[]) => ids.map((id) => ({ id }) as BrowserRow);

describe('stopToast', () => {
  it('reports sandboxes left behind as an error', () => {
    const r = (removed: Array<boolean | null>) => ({
      stopped: removed.length,
      results: removed.map((sandboxRemoved, i) => ({ id: `${i}`, ok: true, sandboxRemoved })),
    });
    expect(stopToast(r([true, null]))).toEqual(['Stopped 2', 'success']);
    expect(stopToast(r([false]))).toEqual(['Stopped 1, 1 sandbox could not be removed', 'error']);
    expect(stopToast(r([false, false]))).toEqual(['Stopped 2, 2 sandboxes could not be removed', 'error']);
  });
});

describe('stopBrowsers', () => {
  it('stops, reports, clears what stopped from the selection, and refreshes', async () => {
    fakeFetch({ body: { stopped: 1, results: [{ id: 'a', ok: true, sandboxRemoved: null }] } });
    const patch = vi.fn();
    const toast = vi.fn();
    const refresh = vi.fn();
    const view = { ...INITIAL_VIEW, stopIds: ['a'], selected: 'a' };
    await stopBrowsers({ apiKey: 'k', view, patch, toast, refresh });
    expect(toast).toHaveBeenCalledWith('Stopped 1', 'success');
    expect(patch.mock.calls).toEqual([
      [{ stopping: true }],
      [{ selected: null }],
      [{ checked: new Set() }],
      [{ stopping: false, stopIds: null }],
    ]);
    expect(refresh).toHaveBeenCalledOnce();
  });

  it('shows a failed stop and still closes the confirmation', async () => {
    fakeFetch({ status: 500, body: { error: 'Server down' } });
    const patch = vi.fn();
    const toast = vi.fn();
    await stopBrowsers({ apiKey: 'k', view: { ...INITIAL_VIEW, stopIds: ['a'] }, patch, toast, refresh: vi.fn() });
    expect(toast).toHaveBeenCalledWith('Server down', 'error');
    expect(patch).toHaveBeenLastCalledWith({ stopping: false, stopIds: null });
  });

  it('does nothing with nothing to stop', async () => {
    const patch = vi.fn();
    await stopBrowsers({ apiKey: 'k', view: INITIAL_VIEW, patch, toast: vi.fn(), refresh: vi.fn() });
    expect(patch).not.toHaveBeenCalled();
  });
});

describe('takeScreenshot', () => {
  it('shows the screenshot, or says why there is none', async () => {
    fakeFetch(
      { body: { ok: true, data: { screenshot: 'data:x' } } },
      { body: { ok: false, error: 'Tab crashed' } },
      {
        body: { ok: false },
      },
    );
    const patch = vi.fn();
    const toast = vi.fn();
    await takeScreenshot('k', 'b1', patch, toast);
    await takeScreenshot('k', 'b1', patch, toast);
    await takeScreenshot('k', 'b1', patch, toast);
    expect(patch).toHaveBeenCalledWith({ shot: { id: 'b1', src: 'data:x' } });
    expect(toast.mock.calls).toEqual([
      ['Tab crashed', 'error'],
      ['No screenshot', 'error'],
    ]);
  });
});

describe('browsersOf', () => {
  it("filters the table by the persona's name, or its id when unnamed", () => {
    const personas = [{ id: 'p1', name: 'Shopper' }] as Persona[];
    expect(browsersOf(personas, 'p1')).toEqual({
      filter: { ...NO_FILTER, persona: 'Shopper' },
      openPersona: null,
      tab: 'browsers',
    });
    expect(browsersOf(personas, 'p2').filter!.persona).toBe('p2');
  });
});

describe('handleSlackReturn', () => {
  it('opens the channel picker after an install that needs a channel, and drops the parameter', () => {
    history.replaceState(null, '', '/dashboard?slack=pick-channel');
    const patch = vi.fn();
    const toast = vi.fn();
    handleSlackReturn(patch, toast);
    expect(patch).toHaveBeenCalledWith({ settingsSection: 'alerts', showSettings: true });
    expect(toast).toHaveBeenCalledWith('Slack connected, choose a channel', 'success');
    expect(window.location.search).toBe('');
  });

  it('confirms a connection and explains a failure', () => {
    const toast = vi.fn();
    history.replaceState(null, '', '/dashboard?slack=connected');
    handleSlackReturn(vi.fn(), toast);
    history.replaceState(null, '', '/dashboard?slack=access_denied');
    handleSlackReturn(vi.fn(), toast);
    handleSlackReturn(vi.fn(), toast);
    expect(toast.mock.calls).toEqual([
      ['Slack connected', 'success'],
      ['Slack install failed: access denied', 'error'],
    ]);
  });
});

describe('pruneSelection', () => {
  it('drops a selected browser that left, unless the list failed to load', () => {
    const patch = vi.fn();
    pruneSelection(rows('b'), { selected: 'a', checked: new Set() }, null, patch);
    pruneSelection(rows('b'), { selected: 'a', checked: new Set() }, 'offline', patch);
    expect(patch.mock.calls).toEqual([[{ selected: null }]]);
  });

  it('unticks browsers that left, and leaves an intact selection alone', () => {
    const patch = vi.fn();
    pruneSelection(rows('a', 'b'), { selected: 'a', checked: new Set(['a', 'z']) }, null, patch);
    pruneSelection(rows('a'), { selected: null, checked: new Set(['a']) }, null, patch);
    expect(patch.mock.calls).toEqual([[{ checked: new Set(['a']) }]]);
  });
});
