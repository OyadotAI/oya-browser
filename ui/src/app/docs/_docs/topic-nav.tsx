/**
 * The docs navigation for a topic page (/docs/<slug>), which shows one part
 * and has no sidebar. The parts' in-text jumps call navigate(); without a
 * provider that is a no-op and each jump would be a dead button, so this one
 * sends a jump to the heading when it is on this page and to the full docs
 * page when it is not.
 */
'use client';

import { useMemo, type PropsWithChildren } from 'react';
import { useRouter } from 'next/navigation';
import { DocsNav, type DocsNavValue } from './nav';

/** Goes to a heading: on this page by its anchor, otherwise on /docs, where every part is. */
function jump(id: string, push: (href: string) => void) {
  if (document.getElementById(id)) window.location.hash = id;
  else push('/docs#' + id);
}

/** Gives a topic page's part a navigate() that works. Nothing is marked active: there is no sidebar to mark. */
export function TopicNav({ children }: PropsWithChildren) {
  const router = useRouter();
  const value = useMemo<DocsNavValue>(
    () => ({ active: '', navigate: (id) => jump(id, (href) => router.push(href)) }),
    [router],
  );
  return <DocsNav.Provider value={value}>{children}</DocsNav.Provider>;
}
