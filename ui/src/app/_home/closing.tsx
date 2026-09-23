/**
 * The last word on the page: what to do next, and where the source is.
 */
import Link from 'next/link';
import { repository } from './content';
import { ArrowRight, ArrowUpRight } from 'lucide-react';
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
      <p>
        Start a browser in a minute. Record the run your team already does by hand. Move the whole thing into your own
        cloud when the data says it has to stay there.
      </p>
      <div className={styles.actions}>
        <Link href="/dashboard" className={styles.primary} data-track="cta_clicked" data-track-label="closing_console">
          Open the console <ArrowUpRight size={17} aria-hidden="true" />
        </Link>
        <a href={repository} className={styles.secondary} data-track="cta_clicked" data-track-label="closing_github">
          Read the source <ArrowRight size={15} aria-hidden="true" />
        </a>
      </div>
    </section>
  );
}
