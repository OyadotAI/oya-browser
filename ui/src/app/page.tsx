/**
 * The public landing page, in the order a buyer reads it: what it is and who it
 * is for, how to download and start, proof, the problems it solves, how it works, why it is different, how it
 * compares, questions, and the next step.
 * Each section lives in app/_home.
 */
import styles from './page.module.css';
import { SiteHeader } from './_home/site-header';
import { Hero } from './_home/hero';
import { GetStarted } from './_home/get-started';
import { Proof } from './_home/proof';
import { Problems } from './_home/problems';
import { HowItWorks } from './_home/how-it-works';
import { Benefits } from './_home/benefits';
import { Compare } from './_home/compare';
import { Faq } from './_home/faq';
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
          <GetStarted />
          <Proof />
          <Problems />
          <HowItWorks />
          <Benefits />
          <Compare />
          <Faq />
          <Closing />
        </main>
        <SiteFooter />
      </div>
    </div>
  );
}
