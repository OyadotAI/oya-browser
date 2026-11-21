/** The landing story starts with a familiar portal task and explains reuse before deployment terminology. */
import Link from 'next/link';
import { Fragment } from 'react';
import { preload } from 'react-dom';
import { COMPLIANCE_CLAIMS, COMPLIANCE_LINE, HERO_POSTER, foundersCall } from './content';
import { ShieldCheck } from 'lucide-react';
import { Arrow } from './arrow';
import { RecordedDemo } from './recorded-demo';
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

/** The linked badge keeps each claim and its trailing dot intact, so a wrapped line never starts with a separator. */
function TrustBadge() {
  return (
    <a className={styles.trustLine} href="/trust" aria-label={COMPLIANCE_LINE}>
      <ShieldCheck size={18} aria-hidden="true" />
      <span>
        {COMPLIANCE_CLAIMS.map((claim, index) => (
          <Fragment key={claim}>
            {index > 0 && ' '}
            <span className={styles.trustClaim}>
              {claim}
              {index < COMPLIANCE_CLAIMS.length - 1 && ' ·'}
            </span>
          </Fragment>
        ))}
      </span>
    </a>
  );
}

/** An editorial story and luminous product stage share one opening composition. */
export function Hero() {
  // On a phone the recording's poster is the largest thing in the first screen; without the hint it queues behind scripts.
  preload(HERO_POSTER, { as: 'image', fetchPriority: 'high' });
  return (
    <section className={`hero ${styles.hero}`} id="hero" aria-labelledby="hero-h">
      <div className={`wrap ${styles.composition}`}>
        <div className={styles.content}>
          <Reveal preset="intro">
            <p className={styles.eyebrow}>For healthtech and RCM technology teams</p>
            <h1 id="hero-h" className={styles.title}>
              <span>Oya completes one payer portal case.</span>{' '}
              <span className={styles.accent}>You keep the steps as code.</span>
            </h1>
          </Reveal>
          <Reveal preset="intro" order={1} className={styles.copy}>
            <p className={styles.lead}>
              Start with a prior authorization, eligibility check or claim-status lookup, and Oya saves the successful
              steps as a playbook you can replay with new inputs.
            </p>
            <Actions place="hero" />
            <TrustBadge />
          </Reveal>
        </div>
        <Reveal preset="artwork" order={1} className={styles.media}>
          <RecordedDemo />
        </Reveal>
      </div>
    </section>
  );
}
