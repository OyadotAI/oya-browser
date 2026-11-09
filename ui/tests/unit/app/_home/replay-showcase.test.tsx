/** An execution gate must survive leaving view so a human-owned example cannot resume itself. */
import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
const environment = vi.hoisted(() => ({ visible: true }));
vi.mock('framer-motion', () => ({ useInView: () => environment.visible, useReducedMotion: () => false }));
import { useReplay } from '@/app/_home/replay-showcase';
import { REPLAY } from '@/app/_home/content';
import { SHOWCASE_TICK_MS } from '@/app/_home/constants';

describe('replay execution gate', () => {
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    environment.visible = true;
  });
  it('stays blocked after offscreen return until explicit ownership resumption', () => {
    vi.useFakeTimers();
    const view = renderHook(({ blocked }) => useReplay(REPLAY, blocked), { initialProps: { blocked: false } });
    act(() => vi.advanceTimersByTime(SHOWCASE_TICK_MS * 2));
    const step = view.result.current.step;
    view.rerender({ blocked: true });
    environment.visible = false;
    view.rerender({ blocked: true });
    environment.visible = true;
    view.rerender({ blocked: true });
    act(() => vi.advanceTimersByTime(SHOWCASE_TICK_MS * 3));
    expect(view.result.current.step).toBe(step);
    expect(vi.getTimerCount()).toBe(0);
    view.rerender({ blocked: false });
    act(() => vi.advanceTimersByTime(SHOWCASE_TICK_MS));
    expect(view.result.current.step).toBe(step + 1);
    view.unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});
