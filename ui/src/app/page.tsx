/**
 * The public landing page. Each section lives in app/_home.
 */
import styles from './page.module.css';
import { SiteHeader } from './_home/site-header';
import { Hero } from './_home/hero';
import { Providers } from './_home/providers';
import { ControlPlane } from './_home/control-plane';
import { ConsoleSection } from './_home/console-section';
import { Developers } from './_home/developers';
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
          <ControlPlane />
          <ConsoleSection />
          <Developers />
        </main>
        <SiteFooter />
      </div>
    </div>
  );
}
