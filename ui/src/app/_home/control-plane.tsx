/**
 * The landing section on the thing that makes Oya different: the automation
 * lives inside the browser instead of driving one from outside.
 */
import Link from 'next/link';
import { ArrowRight, Fingerprint, KeyRound, Network, ShieldCheck } from 'lucide-react';
import styles from '../page.module.css';

/** Your agent, the Oya browser and the site, as a figure. */
function PlaneDiagram() {
  return (
    <figure
      className={styles.planeDiagram}
      aria-label="Your agent connects over the SDK, MCP, CDP or REST. Oya is the browser, with the automation inside it, so the site sees an ordinary Chrome."
    >
      <div className={styles.planeEndpoint}>
        <span className={styles.planeLabel}>YOUR AGENT</span>
        <strong>However it connects.</strong>
        <div className={styles.planeTags}>
          <span>SDK</span>
          <span>MCP</span>
          <span>CDP</span>
          <span>REST</span>
        </div>
      </div>
      <div className={styles.planeConnector} aria-hidden="true">
        <ArrowRight size={18} />
      </div>
      <div className={styles.planeHub}>
        <Network size={25} strokeWidth={1.5} aria-hidden="true" />
        <strong>Oya</strong>
        <span>The browser itself</span>
      </div>
      <div className={styles.planeConnector} aria-hidden="true">
        <ArrowRight size={18} />
      </div>
      <div className={styles.planeEndpoint}>
        <span className={styles.planeLabel}>THE SITE</span>
        <strong>Sees ordinary Chrome.</strong>
        <div className={styles.planeProviders}>
          <span>Headful Chrome</span>
          <span>No Runtime.enable</span>
        </div>
      </div>
    </figure>
  );
}

/** Why a browser we built beats a browser driven from outside. */
export function ControlPlane() {
  return (
    <section className={styles.controlPlane} aria-labelledby="control-plane-title">
      <div className={styles.controlPlaneIntro}>
        <div>
          <p className={styles.eyebrow}>The hard way</p>
          <h2 id="control-plane-title">
            Everyone drives Chrome.
            <br />
            <span>We built the browser.</span>
          </h2>
        </div>
        <div>
          <p>The automation lives inside it, not attached over the debugging protocol.</p>
          <Link href="/docs#anonymity">
            See the numbers <ArrowRight size={16} aria-hidden="true" />
          </Link>
        </div>
      </div>
      <PlaneDiagram />
      <div className={styles.controlPlaneBenefits}>
        <article>
          <Fingerprint size={19} strokeWidth={1.5} aria-hidden="true" />
          <h3>Not flagged as a bot.</h3>
          <p>Chrome’s own emulation sets the device. 0% on CreepJS.</p>
        </article>
        <article>
          <KeyRound size={19} strokeWidth={1.5} aria-hidden="true" />
          <h3>Sign in once.</h3>
          <p>Log in by hand. Every browser after that starts signed in.</p>
        </article>
        <article>
          <ShieldCheck size={19} strokeWidth={1.5} aria-hidden="true" />
          <h3>Prove what happened.</h3>
          <p>A hash-chained audit trail. Allow-list the hosts a browser may reach.</p>
        </article>
      </div>
    </section>
  );
}
