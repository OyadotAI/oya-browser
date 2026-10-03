/**
 * Notes where a visitor came from on the first page they open, so a desktop
 * download can be traced to its ad or link. First-party only: nothing leaves
 * the site but the cookie on the visitor's own requests.
 */
'use client';

import { useEffect } from 'react';
import { usePathname } from 'next/navigation';
import { rememberSource } from '@/lib/visitor-source';
import { isPublicPage } from '@/lib/public-pages';

/** Renders nothing; keeps the source of a visit that lands on a public page. */
export function VisitorSource() {
  const pathname = usePathname();
  useEffect(() => {
    if (isPublicPage(pathname)) rememberSource();
    // Only the page a visit lands on says where it came from.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return null;
}
