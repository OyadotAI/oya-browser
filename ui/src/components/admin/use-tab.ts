/**
 * Which admin tab is open. It lives in `?tab=` so a reload or a shared link
 * lands on the same tab; an unknown value opens the overview.
 */
import { useState } from 'react';
import { TABS, type TabId } from './constants';

/** The tab the address names, or the first one. */
function fromUrl(): TabId {
  if (typeof window === 'undefined') return TABS[0].id;
  const asked = new URLSearchParams(window.location.search).get('tab');
  return TABS.find((t) => t.id === asked)?.id ?? TABS[0].id;
}

/** The open tab, and a way to open another (which rewrites the address without a navigation). */
export function useTab() {
  const [tab, setTab] = useState<TabId>(fromUrl);
  const open = (next: TabId) => {
    setTab(next);
    const url = new URL(window.location.href);
    url.searchParams.set('tab', next);
    window.history.replaceState(window.history.state, '', url);
  };
  return { tab, open };
}
