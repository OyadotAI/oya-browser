/**
 * Runs Meta's ad pixel on public pages: loads it on the first one and counts
 * a page view on each. The layout renders this only when the operator set
 * META_PIXEL_ID; a visitor who only ever opens the console never loads it.
 */
'use client';

import { useEffect } from 'react';
import { usePathname } from 'next/navigation';
import { configureMetaPixel, loadMetaPixel, metaPageView } from '@/lib/meta-pixel';
import { isPublicPage } from '@/lib/public-pages';

/** What the server handed the page. */
type Props = {
  /** The Meta pixel id. */
  id: string;
};

/** Renders nothing; loads the pixel and counts public page views, and tells private pages which pixel a sign-up goes to. */
export function MetaPixel({ id }: Props) {
  const pathname = usePathname();
  useEffect(() => {
    configureMetaPixel(id);
    if (!isPublicPage(pathname)) return;
    loadMetaPixel(id);
    metaPageView(pathname);
  }, [id, pathname]);
  return null;
}
