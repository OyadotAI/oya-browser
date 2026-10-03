/**
 * Unit tests for Meta's pixel: its script is added once, only for a
 * well-formed id, with history tracking and automatic event setup off; page
 * views and sign-ups go through its queue; and a sign-up never waits on a
 * pixel that does not load.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { META_MAX_WAIT_MS } from '@/lib/constants';

/** The window with the pixel's queue. */
type Fbq = { queue: unknown[][]; disablePushState?: boolean };

/** A fresh copy of the module, so each test starts with no pixel loaded. */
async function fresh() {
  vi.resetModules();
  return import('@/lib/meta-pixel');
}

/** The pixel's queued commands. */
const queued = () => (window as unknown as { fbq: Fbq }).fbq.queue;

describe('Meta pixel', () => {
  beforeEach(() => {
    document.head.innerHTML = '';
    delete (window as unknown as { fbq?: unknown }).fbq;
    delete (window as unknown as { _fbq?: unknown })._fbq;
  });
  afterEach(() => vi.useRealTimers());

  it("adds Meta's script once, with history tracking and automatic event setup off", async () => {
    const { loadMetaPixel } = await fresh();
    loadMetaPixel('123456789012345');
    loadMetaPixel('123456789012345');
    expect([...document.querySelectorAll('script')].map((s) => s.src)).toEqual([
      'https://connect.facebook.net/en_US/fbevents.js',
    ]);
    expect((window as unknown as { fbq: Fbq }).fbq.disablePushState).toBe(true);
    expect(queued()).toEqual([
      ['set', 'autoConfig', false, '123456789012345'],
      ['init', '123456789012345'],
    ]);
  });

  it('adds nothing for an id that is not a Meta pixel id', async () => {
    const { loadMetaPixel, validMetaPixelId } = await fresh();
    loadMetaPixel('x"/><script>');
    expect(document.querySelectorAll('script')).toHaveLength(0);
    expect(validMetaPixelId('123456789012345')).toBe(true);
    expect(validMetaPixelId(undefined)).toBe(false);
    expect(validMetaPixelId('12345')).toBe(false);
    expect(validMetaPixelId('abc123456789012')).toBe(false);
  });

  it('counts a view of each page moved to without a reload, once each, and a sign-up as CompleteRegistration', async () => {
    const { loadMetaPixel, metaPageView, metaSignUp } = await fresh();
    loadMetaPixel('123456789012345');
    metaPageView('/');
    metaPageView('/');
    metaPageView('/docs');
    void metaSignUp();
    expect(queued().slice(2)).toEqual([
      ['trackSingle', '123456789012345', 'PageView'],
      ['trackSingle', '123456789012345', 'PageView'],
      ['track', 'CompleteRegistration'],
    ]);
  });

  it('loads the configured pixel for a sign-up on a page that did not load it', async () => {
    const { configureMetaPixel, metaSignUp } = await fresh();
    configureMetaPixel('123456789012345');
    expect(document.querySelectorAll('script')).toHaveLength(0);
    void metaSignUp();
    expect(document.querySelectorAll('script')).toHaveLength(1);
    expect(queued().at(-1)).toEqual(['track', 'CompleteRegistration']);
  });

  it('does nothing, and does not wait, when the pixel is off', async () => {
    const { metaPageView, metaSignUp } = await fresh();
    metaPageView('/');
    await expect(metaSignUp()).resolves.toBeUndefined();
    expect((window as unknown as { fbq?: unknown }).fbq).toBeUndefined();
  });

  it('lets a sign-up go on after a bounded wait when the pixel never loads', async () => {
    vi.useFakeTimers();
    const { loadMetaPixel, metaSignUp } = await fresh();
    loadMetaPixel('123456789012345');
    let done = false;
    void metaSignUp().then(() => (done = true));
    await vi.advanceTimersByTimeAsync(META_MAX_WAIT_MS - 1);
    expect(done).toBe(false);
    await vi.advanceTimersByTimeAsync(1);
    expect(done).toBe(true);
  });
});
