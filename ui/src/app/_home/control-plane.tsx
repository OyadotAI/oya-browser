/**
 * The landing section on the control plane: the routing diagram and its three benefits.
 */
import Link from 'next/link';
import { Activity, ArrowRight, Network, ShieldCheck } from 'lucide-react';
import styles from '../page.module.css';

/** Agents → Oya → browsers, as a figure. */
function PlaneDiagram() {
  return (
    <figure
      className={styles.planeDiagram}
      aria-label="Your agents connect to the Oya control plane, which routes connections to cloud providers or your own Chrome."
    >
      <div className={styles.planeEndpoint}>
        <span className={styles.planeLabel}>YOUR AGENTS</span>
        <strong>The tools you use.</strong>
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
        <span>Route · Observe · Govern</span>
      </div>
      <div className={styles.planeConnector} aria-hidden="true">
        <ArrowRight size={18} />
      </div>
      <div className={styles.planeEndpoint}>
        <span className={styles.planeLabel}>YOUR BROWSERS</span>
        <strong>Wherever work runs.</strong>
        <div className={styles.planeProviders}>
          <span>Oya Cloud</span>
          <span>Your providers</span>
          <span>Your own Chrome</span>
        </div>
      </div>
    </figure>
  );
}

/** Many browsers, one control plane. */
export function ControlPlane() {
  return (
    <section className={styles.controlPlane} aria-labelledby="control-plane-title">
      <div className={styles.controlPlaneIntro}>
        <div>
          <p className={styles.eyebrow}>The browser control plane</p>
          <h2 id="control-plane-title">
            Many browsers.
            <br />
            <span>One control plane.</span>
          </h2>
        </div>
        <div>
          <p>
            Give your agents one place to connect. Choose where browsers run, see what they’re doing, and set the
            limits—all from Oya.
          </p>
          <Link href="/docs#control-plane">
            Explore the control plane <ArrowRight size={16} aria-hidden="true" />
          </Link>
        </div>
      </div>
      <PlaneDiagram />
      <div className={styles.controlPlaneBenefits}>
        <article>
          <Network size={19} strokeWidth={1.5} aria-hidden="true" />
          <h3>Choose where work runs.</h3>
          <p>Route new connections by priority, capacity, or latency. Change providers behind one shared endpoint.</p>
        </article>
        <article>
          <Activity size={19} strokeWidth={1.5} aria-hidden="true" />
          <h3>See what needs attention.</h3>
          <p>See fleet health and usage together. Trace what happened with audit history and session recordings.</p>
        </article>
        <article>
          <ShieldCheck size={19} strokeWidth={1.5} aria-hidden="true" />
          <h3>Scale with clear limits.</h3>
          <p>Set project access, budgets, and quotas. Keep resource use in view as your fleet grows.</p>
        </article>
      </div>
    </section>
  );
}
