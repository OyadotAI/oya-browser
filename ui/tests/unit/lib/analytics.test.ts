/**
 * Unit tests for console analytics: nothing runs until the library is
 * initialised, it is initialised with capture switched off everywhere, and
 * events, identify and reset reach the client once it is there.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

/** A PostHog stand-in that records what it is told. */
const posthog = { init: vi.fn(), capture: vi.fn(), identify: vi.fn(), reset: vi.fn() };
vi.mock('posthog-js', () => ({ default: posthog }));

describe('analytics', () => {
  beforeEach(() => {
    vi.resetModules();
    for (const fn of Object.values(posthog)) fn.mockClear();
  });

  it('does nothing before init, so a console with analytics off never touches the library', async () => {
    const analytics = await import('@/lib/analytics');
    analytics.pageview();
    analytics.event('sign_in_success');
    analytics.identify('u-1', { email: 'ana@example.com' });
    analytics.reset();
    expect(posthog.capture).not.toHaveBeenCalled();
    expect(posthog.identify).not.toHaveBeenCalled();
  });

  it('initialises with autocapture, pageviews and session replay off, and only identified persons', async () => {
    const analytics = await import('@/lib/analytics');
    await analytics.init({ key: 'phc_test', host: 'https://ph.example.test' });
    expect(posthog.init).toHaveBeenCalledWith('phc_test', {
      api_host: 'https://ph.example.test',
      autocapture: false,
      capture_pageview: false,
      capture_pageleave: false,
      disable_session_recording: true,
      disable_external_dependency_loading: true,
      person_profiles: 'identified_only',
    });
  });

  it('forwards pageviews, events, identify and reset once initialised, and initialises only once', async () => {
    const analytics = await import('@/lib/analytics');
    await analytics.init({ key: 'phc_test', host: 'https://ph.example.test' });
    await analytics.init({ key: 'phc_test', host: 'https://ph.example.test' });
    analytics.pageview();
    analytics.event('sign_up_success', { plan: 'free' });
    analytics.identify('u-1', { email: 'ana@example.com' });
    analytics.reset();
    expect(posthog.init).toHaveBeenCalledTimes(1);
    expect(posthog.capture.mock.calls).toEqual([['$pageview'], ['sign_up_success', { plan: 'free' }]]);
    expect(posthog.identify).toHaveBeenCalledWith('u-1', { email: 'ana@example.com' });
    expect(posthog.reset).toHaveBeenCalledTimes(1);
  });
});
