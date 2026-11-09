/** Routing motion is bounded by visibility, pause, reduced motion and component lifetime. */
import { act, renderHook, cleanup } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const environment = vi.hoisted(() => ({ visible: true, reduced: false }));
vi.mock('framer-motion', () => ({ useInView: () => environment.visible, useReducedMotion: () => environment.reduced }));
import { useControlPlaneMotion } from '@/app/_home/use-control-plane-motion';
import { CONTROL_STAGE_COUNT, CONTROL_TICK_MS } from '@/app/_home/constants';

describe('control plane routing motion', () => {
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    environment.visible = true;
    environment.reduced = false;
  });

  it('advances one route stage at a time and stops while paused or offscreen', () => {
    vi.useFakeTimers();
    const view = renderHook(useControlPlaneMotion);
    expect(view.result.current.phase).toBe(CONTROL_STAGE_COUNT - 1);
    act(() => vi.advanceTimersByTime(CONTROL_TICK_MS));
    expect(view.result.current.phase).toBe(0);
    act(() => view.result.current.toggle());
    act(() => vi.advanceTimersByTime(CONTROL_TICK_MS * 2));
    expect(view.result.current.phase).toBe(0);
    expect(vi.getTimerCount()).toBe(0);
    act(() => view.result.current.toggle());
    act(() => vi.advanceTimersByTime(CONTROL_TICK_MS));
    expect(view.result.current.phase).toBe(1);
    environment.visible = false;
    view.rerender();
    expect(vi.getTimerCount()).toBe(0);
    environment.visible = true;
    view.rerender();
    expect(vi.getTimerCount()).toBe(1);
    view.unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it('renders the complete route without a timer when reduced motion is requested', () => {
    vi.useFakeTimers();
    environment.reduced = true;
    const view = renderHook(useControlPlaneMotion);
    expect(view.result.current.still).toBe(true);
    expect(view.result.current.phase).toBe(CONTROL_STAGE_COUNT - 1);
    expect(vi.getTimerCount()).toBe(0);
  });
});
