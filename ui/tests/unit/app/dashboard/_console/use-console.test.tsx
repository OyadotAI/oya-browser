/**
 * Unit tests for the console's composed state: it reopens this tab's project,
 * polls each feed on its interval and pauses while hidden, and only a change
 * of project clears what was shown.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, renderHook } from '@testing-library/react';
import { useConsole } from '@/app/dashboard/_console/use-console';
import { BROWSERS_POLL_MS, FLEET_POLL_MS, PERSONAS_POLL_MS } from '@/app/dashboard/_console/constants';

vi.mock('@/components/dashboard/toast', () => ({ useToast: () => toast }));
const toast = vi.fn();

/** Answers every API path with something sensible. */
const fetchMock = vi.fn(async (url: string) => {
  const body = url.endsWith('/browsers')
    ? [{ id: 'a' }, { id: 'b' }]
    : url.endsWith('/fleet')
      ? { browsers: { commands: 0, errors: 0 } }
      : url.endsWith('/personas')
        ? { personas: [] }
        : { onboarded: true };
  return new Response(JSON.stringify(body));
});
/** Fetch calls to paths ending in `suffix`. */
const calls = (suffix: string) => fetchMock.mock.calls.filter(([u]) => u.endsWith(suffix)).length;

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  vi.stubGlobal('fetch', fetchMock);
  sessionStorage.setItem('oya_console_key', 'k1');
  sessionStorage.setItem('oya_project_id', 'prj');
  history.replaceState(null, '', '/dashboard?browser=a');
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.unstubAllGlobals();
  fetchMock.mockClear();
  sessionStorage.clear();
  history.replaceState(null, '', '/');
});

/** Renders the hook and lets the first loads land. */
async function setup() {
  const hook = renderHook(() => useConsole());
  await act(() => vi.advanceTimersByTimeAsync(0));
  return hook;
}

describe('useConsole', () => {
  it("reopens this tab's project and loads its fleet", async () => {
    const { result } = await setup();
    expect(result.current.apiKey).toBe('k1');
    expect(result.current.project).toBe('prj');
    expect(result.current.browsers).toHaveLength(2);
  });

  it('polls each feed on its own interval', async () => {
    await setup();
    const start = { browsers: calls('/browsers'), fleet: calls('/fleet'), personas: calls('/personas') };
    await act(() => vi.advanceTimersByTimeAsync(PERSONAS_POLL_MS));
    expect(calls('/browsers') - start.browsers).toBe(Math.floor(PERSONAS_POLL_MS / BROWSERS_POLL_MS));
    expect(calls('/fleet') - start.fleet).toBe(PERSONAS_POLL_MS / FLEET_POLL_MS);
    expect(calls('/personas') - start.personas).toBe(1);
  });

  it('pauses polls while the tab is hidden and catches up when shown', async () => {
    await setup();
    Object.defineProperty(document, 'hidden', { value: true, configurable: true });
    act(() => void document.dispatchEvent(new Event('visibilitychange')));
    const before = calls('/browsers');
    await act(() => vi.advanceTimersByTimeAsync(BROWSERS_POLL_MS * 3));
    expect(calls('/browsers')).toBe(before);
    Object.defineProperty(document, 'hidden', { value: false, configurable: true });
    act(() => void document.dispatchEvent(new Event('visibilitychange')));
    expect(calls('/browsers')).toBe(before + 1);
  });

  it('keeps the view when a credential renews for the same project, clears it for another', async () => {
    const { result } = await setup();
    act(() => result.current.patch({ selected: 'a' }));
    act(() => result.current.openProject('k2', 'prj'));
    expect(result.current.view.selected).toBe('a');
    act(() => result.current.openProject('k3', 'other'));
    expect(result.current.view.selected).toBeNull();
    expect(result.current.browsers).toEqual([]);
  });

  it('asks before stopping, and only when there is something to stop', async () => {
    const { result } = await setup();
    act(() => void result.current.requestStop([]));
    expect(result.current.view.stopIds).toBeNull();
    act(() => void result.current.requestStop(['a']));
    expect(result.current.view.stopIds).toEqual(['a']);
  });

  it('drops a browser that left the fleet from the selection', async () => {
    const { result } = await setup();
    act(() => result.current.patch({ selected: 'gone' }));
    await act(() => vi.advanceTimersByTimeAsync(BROWSERS_POLL_MS));
    expect(result.current.view.selected).toBeNull();
  });
});
