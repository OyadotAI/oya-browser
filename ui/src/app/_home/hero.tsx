/**
 * The landing hero: what Oya is, how to start, and where to download it.
 */
import Link from 'next/link';
import WorkflowDiagram from '@/components/workflow-diagram';
import { browserDownloads } from '@/lib/browser-downloads';
import { ArrowRight, ArrowUpRight, Download } from 'lucide-react';
import styles from '../page.module.css';

/** The opening: headline, calls to action, downloads and the playbook diagram. */
export function Hero() {
  return (
    <section className={styles.hero} aria-labelledby="hero-title">
      <div className={styles.heroCopy}>
        <p className={styles.eyebrow}>The browser for AI agents</p>
        <h1 id="hero-title">
          Run it once.
          <br />
          <span>Make it repeatable.</span>
        </h1>
        <p className={styles.heroDescription}>
          Turn browser tasks into reusable playbooks. Replay the work, review repairs, and step in when your agent needs
          you.
        </p>
        <div className={styles.actions}>
          <Link href="/dashboard" className={styles.primary}>
            Start building <ArrowUpRight size={17} aria-hidden="true" />
          </Link>
          <Link href="/docs" className={styles.secondary}>
            Read the docs <ArrowRight size={15} aria-hidden="true" />
          </Link>
        </div>
        <div id="download" className={styles.downloads}>
          <p>Download Oya Browser</p>
          <nav aria-label="Browser downloads">
            {browserDownloads.map(({ platform, architecture, href }) => (
              <a key={platform} href={href} download title={`${platform} · ${architecture}`}>
                <Download size={17} aria-hidden="true" /> {platform}
              </a>
            ))}
          </nav>
          <Link href="/docs#download" className={styles.installHelp}>
            Installation instructions
          </Link>
        </div>
      </div>
      <WorkflowDiagram />
    </section>
  );
}
