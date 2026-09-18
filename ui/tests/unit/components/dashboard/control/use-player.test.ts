/**
 * Unit tests for usePlayer: it loads a recording's frames, shows each as an
 * object URL, advances on the frames' own timing, and pauses on a seek.
 */
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { act, cleanup, renderHook } from '@testing-library/react';
import { usePlayer } from '@/components/dashboard/control/use-player';

/** Three frames 100ms apart. */
const FRAMES = [
  { i: 0, t: 0 },
  { i: 1, t: 100 },
  { i: 2, t: 200 },
];

/** Answers the frame list and each frame as a blob. */
function stubFetch(listOk = true) {
  const fetch = vi.fn(async (url: string) =>
    /\/frames\/\d+$/.test(url)
      ? ({ ok: true, blob: async () => new Blob(['x']) } as Response)
      : listOk
        ? ({ json: async () => ({ frames: FRAMES }) } as Response)
        : Promise.reject(new Error('down')),
  );
  vi.stubGlobal('fetch', fetch);
  return fetch;
}

/** Lets pending promises settle. */
const settle = () => act(async () => {});

describe('usePlayer', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    let n = 0;
    URL.createObjectURL = vi.fn(() => `blob:${n++}`);
    URL.revokeObjectURL = vi.fn();
  });
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('loads the frame list with the key and shows the first frame', async () => {
    const fetch = stubFetch();
    const { result } = renderHook(() => usePlayer('s1', 'k'));
    await settle();
    expect(fetch.mock.calls[0][0]).toMatch(/\/gateway\/recordings\/s1$/);
    expect(result.current.frames).toHaveLength(3);
    expect(result.current.src).toBe('blob:0');
  });

  it('advances by the gap between frames while playing', async () => {
    stubFetch();
    const { result } = renderHook(() => usePlayer('s1', 'k'));
    await settle();
    await act(async () => vi.advanceTimersByTime(100));
    expect(result.current.index).toBe(1);
    expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:0');
  });

  it('pauses when the user seeks', async () => {
    stubFetch();
    const { result } = renderHook(() => usePlayer('s1', 'k'));
    await settle();
    act(() => result.current.seek(2));
    await act(async () => vi.advanceTimersByTime(1000));
    expect(result.current.playing).toBe(false);
    expect(result.current.index).toBe(2);
  });

  it('says so when the recording cannot be loaded', async () => {
    stubFetch(false);
    const { result } = renderHook(() => usePlayer('s1', 'k'));
    await settle();
    expect(result.current.error).toBe('Could not load the recording');
  });
});
