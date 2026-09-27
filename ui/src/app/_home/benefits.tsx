/**
 * Why Oya: each card answers one reason portal automation fails today.
 */
import { Fingerprint, KeyRound, Lock, MoveRight, PenLine, ShieldCheck } from 'lucide-react';
import { BENEFITS } from './content';
import { Reveal } from './motion';
import styles from '../page.module.css';

/** Each benefit's icon, by name. */
const ICONS = {
  fingerprint: Fingerprint,
  key: KeyRound,
  lock: Lock,
  pen: PenLine,
  shield: ShieldCheck,
  move: MoveRight,
};

/** The six benefits under one heading. */
export function Benefits() {
  return (
    <section className={styles.controlPlane} aria-labelledby="benefits-title">
      <Reveal className={styles.controlPlaneIntro}>
        <div>
          <p className={styles.eyebrow}>Why Oya</p>
          <h2 id="benefits-title">
            Everyone drives Chrome.
            <br />
            <span>We built the browser.</span>
          </h2>
        </div>
        <div>
          <p>The automation lives inside it, not attached over the debugging protocol.</p>
        </div>
      </Reveal>
      <div className={styles.controlPlaneBenefits}>
        {BENEFITS.map(({ icon, title, body }, i) => {
          const Icon = ICONS[icon];
          return (
            <Reveal key={title} order={i}>
              <article>
                <span className={styles.benefitIcon}>
                  <Icon size={19} strokeWidth={1.6} aria-hidden="true" />
                </span>
                <h3>{title}</h3>
                <p>{body}</p>
              </article>
            </Reveal>
          );
        })}
      </div>
    </section>
  );
}
