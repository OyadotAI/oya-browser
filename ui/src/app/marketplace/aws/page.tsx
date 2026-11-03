/** AWS buyer completion page; Express owns its form POST and this route renders the clean GET. */
import type { Metadata } from 'next';
import { MarketplaceView } from './marketplace-view';

/** Purchase setup is private account work, never search-index content. */
export const metadata: Metadata = {
  title: 'AWS Marketplace setup',
  robots: { index: false, follow: false },
  referrer: 'no-referrer',
};

/** Reuses Oya authentication while keeping account linking an explicit action. */
export default function MarketplacePage() {
  return <MarketplaceView />;
}
