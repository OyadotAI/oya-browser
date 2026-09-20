/**
 * The landing section on human handoff through the console.
 */
import Image from 'next/image';
import Link from 'next/link';
import { ArrowUpRight, MousePointer2 } from 'lucide-react';
import styles from '../page.module.css';

/** Human handoff: the live view, with a console screenshot. */
export function ConsoleSection() {
  return (
    <section className={styles.control} aria-labelledby="control-title">
      <div className={styles.controlCopy}>
        <h2 id="control-title">
          Autonomous.
          <br />
          <span>Until you’re needed.</span>
        </h2>
        <p>CAPTCHA, MFA, or a question. Open the live view, handle it, and let the run continue.</p>
        <Link href="/docs#dashboard-overview">
          See the console <ArrowUpRight size={16} aria-hidden="true" />
        </Link>
        <div className={styles.controlDetail}>
          <MousePointer2 size={16} aria-hidden="true" />
          <span>Human handoff, built in.</span>
        </div>
      </div>
      <figure className={styles.product}>
        <Image
          src="/oya-browser-poster.jpg"
          alt="Oya console showing three browsers and a live view with human takeover controls"
          width={1920}
          height={1080}
          sizes="(max-width: 960px) calc(100vw - 40px), 760px"
        />
        <figcaption>From the Oya console</figcaption>
      </figure>
    </section>
  );
}
