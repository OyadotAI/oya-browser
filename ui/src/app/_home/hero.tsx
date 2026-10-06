/**
 * The landing hero: who Oya is for and what it does in one line, the two next
 * steps (the desktop download first), and the figure showing who gets through the bot wall.
 */
import Link from 'next/link';
import CopyExample from '@/components/copy-example';
import { foundersCall, heroExample, installCommand } from './content';
import { ArrowDown, ArrowRight, ArrowUpRight } from 'lucide-react';
import { Reveal } from './motion';
import { ReplayShowcase } from './replay-showcase';
import styles from '../page.module.css';

/** Under the showcase, the SDK install line for a developer. */
function InstallBar() {
  return (
    <div className={styles.installBar}>
      <span className={styles.installLine}>$ {installCommand}</span>
      <CopyExample code={`${installCommand}\n\n${heroExample}`} />
      <code className={styles.installPlay}>
        await browser.play(&quot;eligibility-check&quot;, {'{'} memberId {'}'})
      </code>
    </div>
  );
}

/** Download the app, start building, talk to the founders, or read the docs. */
function HeroActions() {
  return (
    <div className={styles.actions}>
      <a href="#download" className={styles.primary} data-track="cta_clicked" data-track-label="hero_download">
        Download the app <ArrowDown size={17} aria-hidden="true" />
      </a>
      <Link href="/dashboard" className={styles.founders} data-track="cta_clicked" data-track-label="hero_start">
        Start building <ArrowUpRight size={17} aria-hidden="true" />
      </Link>
      <a href={foundersCall} className={styles.founders} data-track="cta_clicked" data-track-label="hero_founders">
        Talk to Founders
      </a>
      <Link href="/docs" className={styles.secondary} data-track="cta_clicked" data-track-label="hero_docs">
        Read the docs <ArrowRight size={15} aria-hidden="true" />
      </Link>
    </div>
  );
}

/** The opening: the claim, the next steps, and the product replaying a portal run. */
export function Hero() {
  return (
    <section className={styles.hero} aria-labelledby="hero-title">
      <Reveal className={styles.heroTitle}>
        <p className={styles.eyebrow}>For teams automating payer portals, EHRs and registries</p>
        <h1 id="hero-title">
          The portal has no API.
          <br />
          <span>Your agent still gets&nbsp;in.</span>
        </h1>
      </Reveal>
      <Reveal order={1} className={styles.heroCopy}>
        <p className={styles.heroDescription}>
          Oya is a real browser your agents use without being flagged. Record a task once, and every run after that is
          pure compute: no model, no token bill, signed in like your staff.
        </p>
        <HeroActions />
      </Reveal>
      <Reveal order={2} className={styles.heroSnippetWrap}>
        <ReplayShowcase />
        <InstallBar />
      </Reveal>
    </section>
  );
}
