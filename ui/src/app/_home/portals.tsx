/**
 * The section that names the problem before the product: what portal
 * automation actually costs the team that owns it.
 */
import { PORTAL_TRUTHS } from './content';
import styles from '../page.module.css';

/** The four things that break portal work, numbered. */
export function Portals() {
  return (
    <section className={styles.portals} aria-labelledby="portals-title">
      <div className={styles.portalsIntro}>
        <p className={styles.eyebrow}>The portal reality</p>
        <h2 id="portals-title">
          The work is simple.
          <br />
          <span>Keeping it working is not.</span>
        </h2>
      </div>
      <ol className={styles.portalsList}>
        {PORTAL_TRUTHS.map(({ n, title, body }) => (
          <li key={n}>
            <span className={styles.portalNumber}>{n}</span>
            <h3>{title}</h3>
            <p>{body}</p>
          </li>
        ))}
      </ol>
    </section>
  );
}
