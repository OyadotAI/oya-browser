/**
 * The public landing page. Each section lives in app/_home.
 */
import styles from './page.module.css';
import { SiteHeader } from './_home/site-header';
import { Hero } from './_home/hero';
import { Providers } from './_home/providers';
import { Portals } from './_home/portals';
import { Playbooks } from './_home/playbooks';
import { ControlPlane } from './_home/control-plane';
import { Media } from './_home/media';
import { Compare } from './_home/compare';
import { Developers } from './_home/developers';
import { Closing } from './_home/closing';
import { SiteFooter } from './_home/site-footer';

/** The landing page. */
export default function Home() {
  return (
    <div className={styles.page}>
      <div className={styles.container}>
        <SiteHeader />
        <main>
          <Hero />
          <Providers />
          <Portals />
          <Playbooks />
          <ControlPlane />
          <Media />
          <Compare />
          <Developers />
          <Closing />
        </main>
        <SiteFooter />
      </div>
    </div>
  );
}
