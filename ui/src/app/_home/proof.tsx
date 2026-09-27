/**
 * The numbers under the hero, and the browsers Oya runs on: the proof a buyer
 * looks for before reading any further.
 */
import { PROOF, PROVIDERS } from './content';
import { CountUp, Reveal } from './motion';
import styles from '../page.module.css';

/** Four measured numbers, then the providers. */
export function Proof() {
  return (
    <section aria-label="Oya in numbers">
      <dl className={styles.proof}>
        {PROOF.map(({ value, suffix, label }, i) => (
          <Reveal key={label} order={i} className={styles.proofItem}>
            <dt>
              <CountUp to={value} suffix={suffix} />
            </dt>
            <dd>{label}</dd>
          </Reveal>
        ))}
      </dl>
      <div className={styles.providers}>
        <p>Your browser. Your choice.</p>
        <ul>
          {PROVIDERS.map((provider) => (
            <li key={provider}>{provider}</li>
          ))}
        </ul>
      </div>
    </section>
  );
}
