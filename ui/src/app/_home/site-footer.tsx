/** The footer every public page shares: the landing's own closing, so footers match across pages. */
import { Closing } from './closing';
import { SiteChrome } from './site-chrome';

/** The landing footer's links (each page ends with its own invitation), rendered with the landing's tokens on any public page. */
export function SiteFooter() {
  return (
    <SiteChrome>
      <Closing invitation={false} />
    </SiteChrome>
  );
}
