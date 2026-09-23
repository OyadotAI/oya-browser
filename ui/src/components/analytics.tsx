/**
 * Mounts product analytics when the operator turned it on. The layout renders
 * this only when both PostHog settings are set on the server, so a console
 * with analytics off has nothing here at all. Pageviews follow the route,
 * tagged clicks count on public pages, and the signed-in person is identified
 * when they sign in and forgotten when they sign out.
 */
'use client';

import { useEffect, useState } from 'react';
import { usePathname } from 'next/navigation';
import { init, pageview, identify, reset, trackedClick } from '@/lib/analytics';
import { isPublicPage } from '@/lib/public-pages';
import { useAuth } from '@/components/auth-provider';

/** What the server handed the page. */
type Props = {
  /** The PostHog project's write key. */
  posthogKey: string;
  /** The PostHog host. */
  host: string;
};

/** Identifies the person on sign-in and forgets them on sign-out, once the library is there to tell. */
function useAnalyticsIdentity(ready: boolean) {
  const { user } = useAuth();
  useEffect(() => {
    if (!ready) return;
    if (user) identify(user.id, { email: user.email });
    else reset();
  }, [ready, user]);
}

/** Records a pageview whenever the route changes, and the first one once the library has loaded. */
function usePageviews(ready: boolean) {
  const pathname = usePathname();
  useEffect(() => {
    if (ready) pageview();
  }, [ready, pathname]);
}

/** Counts clicks on elements a public page tagged with data-track; the console tags nothing and is never listened on. */
function useTrackedClicks(ready: boolean) {
  const pathname = usePathname();
  useEffect(() => {
    if (!ready || !isPublicPage(pathname)) return;
    const onClick = (e: MouseEvent) => trackedClick(e.target);
    document.addEventListener('click', onClick, { capture: true });
    return () => document.removeEventListener('click', onClick, { capture: true });
  }, [ready, pathname]);
}

/** Loads the library once; `ready` turns true when it can capture, so nothing fires into the void before then. */
function useAnalyticsReady(posthogKey: string, host: string) {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    void init({ key: posthogKey, host }).then(() => setReady(true));
  }, [posthogKey, host]);
  return ready;
}

/** Renders nothing; its effects are the whole point. */
export function Analytics({ posthogKey, host }: Props) {
  const ready = useAnalyticsReady(posthogKey, host);
  usePageviews(ready);
  useTrackedClicks(ready);
  useAnalyticsIdentity(ready);
  return null;
}
