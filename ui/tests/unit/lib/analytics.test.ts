/**
 * Unit tests for site and console analytics: nothing runs until the library is
 * initialised, it is initialised with automatic capture off, a tagged click
 * says only its name and labels, and events, identify and reset reach the
 * client once it is there.
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

  it('initialises with autocapture and session replay off, time on page on, and only identified persons', async () => {
    const analytics = await import('@/lib/analytics');
    await analytics.init({ key: 'phc_test', host: 'https://ph.example.test' });
    expect(posthog.init).toHaveBeenCalledWith('phc_test', {
      api_host: 'https://ph.example.test',
      autocapture: false,
      capture_pageview: false,
      capture_pageleave: true,
      disable_session_recording: true,
      disable_external_dependency_loading: true,
      person_profiles: 'identified_only',
    });
  });

  it('counts a click on a tagged element by its name and labels only, never its text', async () => {
    const analytics = await import('@/lib/analytics');
    await analytics.init({ key: 'phc_test', host: 'https://ph.example.test' });
    document.body.innerHTML =
      '<a data-track="download_clicked" data-track-label="macOS"><span id="inner">Download for sk_live_secret</span></a><p id="plain">x</p>';
    analytics.trackedClick(document.getElementById('inner'));
    analytics.trackedClick(document.getElementById('plain'));
    analytics.trackedClick(null);
    expect(posthog.capture.mock.calls).toEqual([
      ['download_clicked', { label: 'macOS', place: '', path: window.location.pathname }],
    ]);
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
