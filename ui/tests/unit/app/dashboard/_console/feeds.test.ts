/**
 * Unit tests for the console's feeds: each skips while hidden or without a
 * credential, and drops a response that arrives after the credential changed.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { loadBrowsers, loadFleet, loadKeyConfig, loadPersonas, type Liveness } from '@/app/dashboard/_console/feeds';
import { fakeFetch } from '../../../support';

afterEach(() => vi.unstubAllGlobals());

/** Liveness for credential `key`. */
const live = (key = 'k', hidden = false): Liveness => ({ keyRef: { current: key }, hidden: { current: hidden } });

describe('loadBrowsers', () => {
  it('shows the rows and clears the error', async () => {
    fakeFetch({ body: [{ id: 'a' }] });
    const sink = { setBrowsers: vi.fn(), setLoadError: vi.fn() };
    await loadBrowsers('k', live(), sink);
    expect(sink.setBrowsers).toHaveBeenCalledWith([{ id: 'a' }]);
    expect(sink.setLoadError).toHaveBeenCalledWith(null);
  });

  it('keeps the last rows and says why when the refresh fails', async () => {
    fakeFetch({ status: 500, body: { error: 'down' } });
    const sink = { setBrowsers: vi.fn(), setLoadError: vi.fn() };
    await loadBrowsers('k', live(), sink);
    expect(sink.setBrowsers).not.toHaveBeenCalled();
    expect(sink.setLoadError).toHaveBeenCalledWith('down');
  });

  it("drops a previous credential's answer, and polls nothing while hidden or keyless", async () => {
    const fn = fakeFetch({ body: [] });
    const sink = { setBrowsers: vi.fn(), setLoadError: vi.fn() };
    await loadBrowsers('old', live('new'), sink);
    await loadBrowsers('k', live('k', true), sink);
    await loadBrowsers('', live(''), sink);
    expect(fn).toHaveBeenCalledOnce();
    expect(sink.setBrowsers).not.toHaveBeenCalled();
  });
});

describe('loadFleet', () => {
  it('computes the throughput from the previous poll', async () => {
    fakeFetch({ body: { browsers: { commands: 10, errors: 0 } } }, { body: { browsers: { commands: 20, errors: 5 } } });
    const sink = { setFleet: vi.fn(), setRate: vi.fn(), sample: { current: null } };
    vi.spyOn(Date, 'now').mockReturnValueOnce(0).mockReturnValueOnce(60_000);
    await loadFleet('k', live(), sink);
    expect(sink.setRate).not.toHaveBeenCalled();
    await loadFleet('k', live(), sink);
    expect(sink.setRate).toHaveBeenCalledWith({ commandsPerMin: 10, errorPct: 50 });
  });

  it('leaves the strip alone when the poll fails', async () => {
    fakeFetch({ status: 500 });
    const sink = { setFleet: vi.fn(), setRate: vi.fn(), sample: { current: null } };
    await loadFleet('k', live(), sink);
    expect(sink.setFleet).not.toHaveBeenCalled();
  });
});

describe('loadPersonas', () => {
  it('shows the personas, an empty list when absent, and keeps the last on failure', async () => {
    fakeFetch({ body: { personas: [{ id: 'p' }] } }, { body: {} }, { status: 500 });
    const set = vi.fn();
    await loadPersonas('k', live(), set);
    await loadPersonas('k', live(), set);
    await loadPersonas('k', live(), set);
    expect(set.mock.calls).toEqual([[[{ id: 'p' }]], [[]]]);
  });
});

describe('loadKeyConfig', () => {
  it('decides the wizard once per project, not on later refreshes', async () => {
    fakeFetch({ body: { onboarded: false } });
    const sink = { setConfig: vi.fn(), setOnboarding: vi.fn(), decidedFor: { current: null } };
    await loadKeyConfig('k', 'prj', live(), sink);
    await loadKeyConfig('k', 'prj', live(), sink);
    await loadKeyConfig('k', 'other', live(), sink);
    expect(sink.setOnboarding.mock.calls).toEqual([[true], [true]]);
    expect(sink.setConfig).toHaveBeenCalledTimes(3);
  });

  it('clears the config without a credential, and ignores a stale answer', async () => {
    const fn = fakeFetch({ body: { onboarded: true } });
    const sink = { setConfig: vi.fn(), setOnboarding: vi.fn(), decidedFor: { current: null } };
    await loadKeyConfig('', null, live(''), sink);
    await loadKeyConfig('old', null, live('new'), sink);
    expect(sink.setConfig.mock.calls).toEqual([[null]]);
    expect(fn).toHaveBeenCalledOnce();
  });
});
