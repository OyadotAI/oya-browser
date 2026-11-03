/** Preserves the buyer's sense of place while reusing existing account authentication. */
'use client';
import { useSyncExternalStore } from 'react';
import { pendingMarketplace } from '@/lib/marketplace';

/** Navigation remounts auth pages; the hint needs no background listener. */
const subscribe = () => () => {};
/** Server rendering cannot read the browser's hint. */
const serverHint = () => false;

/** A hydration-safe context line, without a second signup or sign-in flow. */
export function MarketplaceContext() {
  const pending = useSyncExternalStore(subscribe, pendingMarketplace, serverHint);
  return pending ? (
    <p className="mb-6 text-sm text-text-muted">Continue your AWS Marketplace setup after signing in.</p>
  ) : null;
}
