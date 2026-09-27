/**
 * The last word on the page: the two next steps, once more.
 */
import Link from 'next/link';
import { ArrowUpRight } from 'lucide-react';
import { foundersCall } from './content';
import styles from '../page.module.css';

/** The closing call to action. */
export function Closing() {
  return (
    <section className={styles.closing} aria-labelledby="closing-title">
      <h2 id="closing-title">
        Stop writing integrations
        <br />
        <span>for portals that don&apos;t want you.</span>
      </h2>
      <p>Start a browser in a minute, record the run your team already does by hand, and replay it on every case.</p>
      <div className={styles.actions}>
        <Link href="/dashboard" className={styles.primary} data-track="cta_clicked" data-track-label="closing_start">
          Start building <ArrowUpRight size={17} aria-hidden="true" />
        </Link>
        <a href={foundersCall} className={styles.founders} data-track="cta_clicked" data-track-label="closing_founders">
          Talk to Founders
        </a>
      </div>
    </section>
  );
}
