/** The landing story starts with a familiar portal task and explains reuse before deployment terminology. */
import Link from 'next/link';
import Image from 'next/image';
import { COMPLIANCE_LINE, foundersCall } from './content';
import { ShieldCheck } from 'lucide-react';
import { Arrow } from './arrow';
import { HERO_ART_HEIGHT, HERO_ART_WIDTH } from './constants';
import styles from './hero.module.css';
import { Reveal } from '@/components/ui/reveal';

/** The two actions: a demo first, since regulated buyers evaluate before they build, then the console. */
export function Actions({ place }: { /** Where the buttons sit, for analytics. */ place: string }) {
  return (
    <div className="btn-row">
      <a
        className="btn btn--primary"
        href={foundersCall}
        data-track="cta_clicked"
        data-track-label={`${place}_founders`}
      >
        Book a demo
        <Arrow />
      </a>
      <Link
        className="btn btn--secondary"
        href="/dashboard"
        data-track="cta_clicked"
        data-track-label={`${place}_start`}
      >
        Start building
      </Link>
    </div>
  );
}

/** Original glass artwork echoes Oya's interlocking rings without implying a live product view. */
function HeroArtwork() {
  return (
    <div className={styles.artwork} aria-hidden="true">
      <Reveal preset="artwork">
        <Image
          src="/oya-routing-sculpture.png"
          alt=""
          width={HERO_ART_WIDTH}
          height={HERO_ART_HEIGHT}
          sizes="(max-width: 899px) 100vw, 58vw"
          preload
        />
      </Reveal>
    </div>
  );
}

/** A two-scale headline leads into an asymmetric product-art composition and the existing actions. */
export function Hero() {
  return (
    <section className={`hero ${styles.hero}`} id="hero" aria-labelledby="hero-h">
      <div className="wrap">
        <Reveal preset="intro">
          <p className={styles.eyebrow}>For healthtech and RCM technology teams</p>
          <h1 id="hero-h" className={styles.title}>
            <span className={styles.intro}>Turn repeated portal work into</span>{' '}
            <span className={styles.accent}>a workflow you can reuse.</span>
          </h1>
        </Reveal>
        <div className={styles.composition}>
          <Reveal preset="intro" order={1} className={styles.copy}>
            <p className={styles.lead}>
              Your team already knows how to work the portal. Have Oya complete one task on your desktop, save its steps
              as readable code, and run those steps again with new inputs.
            </p>
            <p className={styles.scope}>
              Use synthetic data while developing on desktop. For protected health information (PHI), use Oya Cloud or
              the enterprise edition under a Business Associate Agreement (BAA).
            </p>
            <Actions place="hero" />
            <a className={styles.trustLine} href="/trust">
              <ShieldCheck size={18} aria-hidden="true" />
              {COMPLIANCE_LINE}
            </a>
          </Reveal>
          <HeroArtwork />
        </div>
      </div>
    </section>
  );
}
