/**
 * Unit tests for live-view geometry and status: screen points map to page
 * pixels through object-contain's letterbox, and the status line reads freshness.
 */
import { describe, it, expect } from 'vitest';
import { toPagePoint } from '@/components/dashboard/live/geometry';
import { frameStatus, staleSeconds } from '@/components/dashboard/live/status';

/** An image of `nw`×`nh` natural pixels drawn in a `w`×`h` box at (10, 20). */
function image(nw: number, nh: number, w: number, h: number) {
  const rect = { left: 10, top: 20, width: w, height: h } as DOMRect;
  return { naturalWidth: nw, naturalHeight: nh, getBoundingClientRect: () => rect } as HTMLImageElement;
}

describe('toPagePoint', () => {
  it('returns null before a frame has loaded', () => {
    expect(toPagePoint(null, 0, 0)).toBeNull();
    expect(toPagePoint(image(0, 0, 100, 100), 50, 50)).toBeNull();
  });

  it('scales a displayed point to the page and keeps the element-local point', () => {
    expect(toPagePoint(image(1000, 500, 500, 250), 110, 70)).toEqual({ x: 200, y: 100, localX: 100, localY: 50 });
  });

  it('removes the letterbox when the box is taller than the image', () => {
    // 1000×500 in 500×500: drawn 500×250, centred with 125px bars above and below.
    expect(toPagePoint(image(1000, 500, 500, 500), 10, 20 + 125)).toMatchObject({ x: 0, y: 0 });
  });

  it('ignores points in the letterbox bars', () => {
    expect(toPagePoint(image(1000, 500, 500, 500), 50, 30)).toBeNull();
  });
});

describe('frameStatus', () => {
  it('says connecting until the first frame', () => expect(frameStatus(null, 5, 0)).toBe('connecting'));
  it('shows the frame rate when above one frame a second', () => expect(frameStatus('f', 4, 0)).toBe('4 fps'));
  it('calls a slow but recent stream live', () => expect(frameStatus('f', 1, 2000)).toBe('live'));
  it('calls an old or unknown frame idle', () => {
    expect(frameStatus('f', 0, 4000)).toBe('idle');
    expect(frameStatus('f', 0, null)).toBe('idle');
  });
});

describe('staleSeconds', () => {
  it('stays quiet for fresh frames', () => {
    expect(staleSeconds(null)).toBeNull();
    expect(staleSeconds(5000)).toBeNull();
  });
  it('rounds the age of an old frame to seconds', () => expect(staleSeconds(12_400)).toBe(12));
});
