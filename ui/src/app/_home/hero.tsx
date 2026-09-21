/**
 * The landing hero: the portal problem in one line, the install and the five
 * lines that solve it, and the figure showing who gets through the bot wall.
 */
import Link from 'next/link';
import CopyExample from '@/components/copy-example';
import SyntaxCode from '@/components/ui/syntax-code';
import { BotWall } from './bot-wall';
import { heroExample, installCommand } from './content';
import { browserDownloads } from '@/lib/browser-downloads';
import { ArrowRight, ArrowUpRight, Download } from 'lucide-react';
import styles from '../page.module.css';

/** The install line and the five lines that follow it. */
function HeroSnippet() {
  return (
    <div className={styles.heroSnippet}>
      <div className={styles.codeHeader}>
        <span className={styles.installLine}>$ {installCommand}</span>
        <CopyExample code={`${installCommand}\n\n${heroExample}`} />
      </div>
      <pre tabIndex={0} aria-label="Starting a browser and replaying a recorded portal run">
        <SyntaxCode code={heroExample} language="typescript" />
      </pre>
    </div>
  );
}

/** The desktop build, for the people who sign in by hand. */
function Downloads() {
  return (
    <div id="download" className={styles.downloads}>
      <p>Or sign in by hand, once</p>
      <nav aria-label="Browser downloads">
        {browserDownloads.map(({ platform, architecture, href }) => (
          <a key={platform} href={href} download title={`${platform} · ${architecture}`}>
            <Download size={17} aria-hidden="true" /> {platform}
          </a>
        ))}
      </nav>
    </div>
  );
}

/** The opening: the claim, the code, and the figure. */
export function Hero() {
  return (
    <section className={styles.hero} aria-labelledby="hero-title">
      <div className={styles.heroCopy}>
        <p className={styles.eyebrow}>Browser infrastructure for portal automation</p>
        <h1 id="hero-title">
          The portal has no API.
          <br />
          <span>Your agent still gets in.</span>
        </h1>
        <p className={styles.heroDescription}>
          Payer portals, EHRs and registries are built to stop bots. Oya is a real browser with the automation inside
          it: your agents sign in like staff, and a run recorded once replays forever with no model in the loop.
        </p>
        <div className={styles.actions}>
          <Link href="/dashboard" className={styles.primary}>
            Start building <ArrowUpRight size={17} aria-hidden="true" />
          </Link>
          <Link href="/docs" className={styles.secondary}>
            Read the docs <ArrowRight size={15} aria-hidden="true" />
          </Link>
        </div>
        <Downloads />
      </div>
      <BotWall />
      <HeroSnippet />
    </section>
  );
}
