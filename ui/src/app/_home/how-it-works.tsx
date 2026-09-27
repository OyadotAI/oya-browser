/**
 * How it works, in the three steps a team takes, and the walkthrough that
 * shows all three.
 */
import { STEPS } from './content';
import { Reveal } from './motion';
import { Walkthrough } from './walkthrough';
import styles from '../page.module.css';

/** Ask, save, replay, then the walkthrough video. */
export function HowItWorks() {
  return (
    <section id="how-it-works" className={styles.media} aria-labelledby="how-title">
      <Reveal className={styles.mediaIntro}>
        <p className={styles.eyebrow}>How it works</p>
        <h2 id="how-title">
          Do it once.
          <br />
          <span>Replay it forever.</span>
        </h2>
      </Reveal>
      <Reveal>
        <ol className={styles.steps}>
          {STEPS.map(({ n, title, body }) => (
            <li key={n}>
              <span className={styles.stepNumber}>{n}</span>
              <h3>{title}</h3>
              <p>{body}</p>
            </li>
          ))}
        </ol>
      </Reveal>
      <Walkthrough />
    </section>
  );
}
