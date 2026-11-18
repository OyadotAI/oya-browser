/** Progressive motion never hides server content and releases every browser resource on interaction or teardown. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import Link from 'next/link';
import { renderToStaticMarkup } from 'react-dom/server';
import { attachDisclosure, attachReveal } from '@/components/ui/motion';
import { Reveal } from '@/components/ui/reveal';
import { Disclosure } from '@/components/ui/disclosure';

const animation = vi.hoisted(() => ({ complete: vi.fn(), stop: vi.fn(), start: vi.fn() }));
vi.mock('framer-motion', () => ({ animate: animation.start }));
let changed: (() => void) | undefined;
let observed: IntersectionObserverCallback;
let preference: {
  matches: boolean;
  addEventListener: ReturnType<typeof vi.fn>;
  removeEventListener: ReturnType<typeof vi.fn>;
};
const observe = vi.fn(),
  disconnect = vi.fn();

/** The observer only fires when the test explicitly moves a region into view. */
class Observer {
  constructor(callback: IntersectionObserverCallback) {
    observed = callback;
  }
  observe = observe;
  disconnect = disconnect;
}

/** Reports one threshold crossing without requiring browser layout in jsdom. */
const enter = (ratio = 1) =>
  observed(
    [{ isIntersecting: true, intersectionRatio: ratio } as IntersectionObserverEntry],
    {} as IntersectionObserver,
  );

beforeEach(() => {
  vi.clearAllMocks();
  document.body.innerHTML = '<div id="region"><a href="#next">Read more</a></div>';
  animation.start.mockImplementation(() => ({ complete: animation.complete, stop: animation.stop }));
  preference = {
    matches: false,
    addEventListener: vi.fn((_type, callback) => {
      changed = callback;
    }),
    removeEventListener: vi.fn(),
  };
  vi.stubGlobal('matchMedia', (query: string) => (query.includes('prefers-reduced') ? preference : { matches: false }));
  vi.stubGlobal('IntersectionObserver', Observer);
});
afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});
const region = () => document.getElementById('region')!;

describe('progressive reveals', () => {
  it('renders complete readable server HTML before any browser enhancement', () => {
    const html = renderToStaticMarkup(
      <Reveal>
        <h2>Visible first</h2>
        <Link href="/docs">Documentation</Link>
      </Reveal>,
    );
    expect(html).toContain('Visible first');
    expect(html).toContain('href="/docs"');
    expect(html).not.toMatch(/opacity|hidden|transform/);
  });

  it('fails open without observer or media APIs and on observer setup errors', () => {
    vi.stubGlobal('IntersectionObserver', undefined);
    const absent = attachReveal(region(), 'section');
    absent();
    vi.stubGlobal(
      'IntersectionObserver',
      class {
        constructor() {
          throw new Error('Unavailable');
        }
      },
    );
    const failed = attachReveal(region(), 'section');
    failed();
    expect(preference.removeEventListener).toHaveBeenCalled();
    vi.stubGlobal('matchMedia', undefined);
    expect(() => attachReveal(region(), 'section')()).not.toThrow();
    expect(animation.start).not.toHaveBeenCalled();
    expect(region().style.opacity).toBe('');
  });

  it('starts once at the visible threshold and never replays on re-entry', () => {
    const release = attachReveal(region(), 'section');
    enter(0.1);
    expect(animation.start).not.toHaveBeenCalled();
    enter();
    enter();
    expect(animation.start).toHaveBeenCalledTimes(1);
    expect(disconnect).toHaveBeenCalled();
    release();
  });

  it.each(['focusin', 'pointerdown'])('settles on %s before entry and prevents later playback', (type) => {
    const release = attachReveal(region(), 'section');
    region()
      .querySelector('a')!
      .dispatchEvent(new Event(type, { bubbles: true }));
    enter();
    expect(animation.start).not.toHaveBeenCalled();
    release();
  });

  it('completes an active animation when reduced motion is enabled', () => {
    const release = attachReveal(region(), 'artwork');
    enter();
    preference.matches = true;
    changed?.();
    expect(animation.complete).toHaveBeenCalledTimes(1);
    expect(animation.stop).toHaveBeenCalledTimes(1);
    expect(region().style.transform).toBe('');
    release();
  });

  it('does not animate content focused before its hydration effect attaches', () => {
    region().querySelector('a')!.focus();
    const release = attachReveal(region(), 'section');
    expect(observe).not.toHaveBeenCalled();
    expect(animation.start).not.toHaveBeenCalled();
    release();
  });

  it('does not start motion when the initial preference is reduced', () => {
    preference.matches = true;
    attachReveal(region(), 'section')();
    expect(observe).not.toHaveBeenCalled();
    expect(animation.start).not.toHaveBeenCalled();
  });

  it('cleans up observation, preference and interaction listeners and ignores late callbacks', () => {
    const element = region(),
      remove = vi.spyOn(element, 'removeEventListener');
    const release = attachReveal(element, 'section');
    enter();
    release();
    enter();
    changed?.();
    element.dispatchEvent(new Event('focusin'));
    expect(animation.start).toHaveBeenCalledTimes(1);
    expect(preference.removeEventListener).toHaveBeenCalledWith('change', changed);
    expect(remove.mock.calls.map(([name]) => name)).toEqual(expect.arrayContaining(['focusin', 'pointerdown']));
    expect(animation.stop).toHaveBeenCalledTimes(1);
  });

  it('returns to final content if the animation engine fails', () => {
    animation.start.mockImplementation(() => {
      throw new Error('Animation unavailable');
    });
    const release = attachReveal(region(), 'section');
    expect(() => enter()).not.toThrow();
    expect(region().style.opacity).toBe('');
    release();
  });
});

describe('native disclosure motion', () => {
  it('keeps the native summary and complete detail in server HTML', () => {
    const html = renderToStaticMarkup(
      <Disclosure summary="Technical details">
        <p>Complete explanation.</p>
      </Disclosure>,
    );
    expect(html).toContain('<summary>');
    expect(html).toContain('Complete explanation.');
    const frame = document.createElement('div');
    frame.innerHTML = html;
    expect(frame.querySelector('details')?.hasAttribute('aria-hidden')).toBe(false);
    expect(frame.querySelector('.motion-disclosure-body')?.hasAttribute('hidden')).toBe(false);
  });

  it('animates opening content without delaying close, and releases its toggle listener', () => {
    const details = document.createElement('details'),
      body = document.createElement('div');
    details.append(body);
    const remove = vi.spyOn(details, 'removeEventListener');
    const release = attachDisclosure(details, body);
    details.open = true;
    details.dispatchEvent(new Event('toggle'));
    expect(animation.start).toHaveBeenCalledTimes(1);
    details.open = false;
    details.dispatchEvent(new Event('toggle'));
    expect(animation.complete).toHaveBeenCalledTimes(1);
    expect(details.open).toBe(false);
    release();
    expect(remove).toHaveBeenCalledWith('toggle', expect.any(Function));
  });
});
