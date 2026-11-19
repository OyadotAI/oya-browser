/** The header every public page shares: the landing's own navigation, so menus match across pages. */
import { Nav } from './nav';
import { SiteChrome } from './site-chrome';

/** The landing navigation, rendered with the landing's tokens on any public page. */
export function SiteHeader() {
  return (
    <SiteChrome>
      <Nav />
    </SiteChrome>
  );
}
