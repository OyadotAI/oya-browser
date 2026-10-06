/**
 * Get the desktop app: a download button per platform and the numbered steps
 * from download to a first task, so nobody has to hunt for either. Sits right
 * under the hero; the header's Download button lands here.
 */
import Link from 'next/link';
import { Download } from 'lucide-react';
import { browserDownloads } from '@/lib/browser-downloads';
import { START_STEPS } from './content';
import { Reveal } from './motion';
import styles from '../page.module.css';

/** One button per platform; the architecture shows on hover. */
function DownloadButtons() {
  return (
    <nav aria-label="Browser downloads" className={styles.getDownloads}>
      {browserDownloads.map(({ platform, architecture, href }) => (
        <a
          key={platform}
          href={href}
          download
          title={architecture}
          data-track="download_clicked"
          data-track-label={platform}
        >
          <Download size={15} aria-hidden="true" /> Download for {platform}
        </a>
      ))}
    </nav>
  );
}

/** The download section and the steps to start using the app. */
export function GetStarted() {
  return (
    <section id="download" className={styles.getStarted} aria-labelledby="get-title">
      <Reveal className={styles.getHead}>
        <div>
          <p className={styles.eyebrow}>Get started in 2 minutes</p>
          <h2 id="get-title">Download Oya Browser. It&apos;s free.</h2>
        </div>
        <DownloadButtons />
      </Reveal>
      <Reveal order={1}>
        <ol className={styles.getSteps} aria-label="How to start">
          {START_STEPS.map(({ n, title, body }) => (
            <li key={n}>
              <span className={styles.getNumber}>{n}</span>
              <strong>{title}</strong>
              <p>{body}</p>
            </li>
          ))}
        </ol>
        <p className={styles.getHelp}>
          Stuck? <Link href="/docs#download">Read the install guide</Link>.
        </p>
      </Reveal>
    </section>
  );
}
