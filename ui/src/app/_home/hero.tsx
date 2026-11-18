/**
 * Leads with the platform loop: build a browser automation on the desktop, deploy it anywhere,
 * replay it with no model. No other browser vendor can say "anywhere", because each one is the
 * cloud; Oya is the layer above them. The eyebrow and the trust line keep healthcare, Oya's first
 * market, as the proof. Its actions lead into the existing demo and developer entry point.
 */
import Link from 'next/link';
import { COMPLIANCE_LINE, foundersCall } from './content';
import { ShieldCheck } from 'lucide-react';
import { Arrow } from './arrow';
import styles from './landing.module.css';

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

/** The hero section. */
export function Hero() {
  return (
    <section className="hero hero--product" id="hero" data-enter aria-labelledby="hero-h">
      <div className="wrap">
        <div className="hero-text hero__in">
          <div className="hero__head">
            <div>
              <p className={styles.eyebrow}>For healthtech and RCM technology teams</p>
              <h1 id="hero-h">
                Build browser automations on your desktop.{' '}
                <span className={styles.heroAccent}>Deploy them anywhere.</span>
              </h1>
              <p className="hero__lead">
                An agent does the task once. Oya keeps it as Playwright code that replays with no model, on Oya Cloud or
                your own infrastructure.
              </p>
            </div>
            <Actions place="hero" />
          </div>
          <a className={styles.trustLine} href="/trust">
            <ShieldCheck size={18} aria-hidden="true" />
            {COMPLIANCE_LINE}
          </a>
        </div>
      </div>
    </section>
  );
}
