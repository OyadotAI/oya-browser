/**
 * Loads RB2B once a visitor is on a public page. The layout renders this only
 * when the operator set RB2B_ID; a visitor who only ever opens the console
 * never loads it.
 */
'use client';

import { useEffect } from 'react';
import { usePathname } from 'next/navigation';
import { loadRb2b } from '@/lib/rb2b';
import { isPublicPage } from '@/lib/public-pages';

/** What the server handed the page. */
type Props = {
  /** The RB2B account id. */
  id: string;
};

/** Renders nothing; loads RB2B on the first public page. */
export function Rb2b({ id }: Props) {
  const pathname = usePathname();
  useEffect(() => {
    if (isPublicPage(pathname)) loadRb2b(id);
  }, [id, pathname]);
  return null;
}
