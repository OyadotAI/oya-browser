/**
 * The three problems Oya was built to solve, each on a card: how it goes today,
 * and how it goes with Oya. The bot-wall figure beside the heading shows the
 * first one happening.
 */
import { BotWall } from './bot-wall';
import { PROBLEMS } from './content';
import { Reveal } from './motion';
import styles from '../page.module.css';

/** Three numbered problems and their answers. */
export function Problems() {
  return (
    <section className={styles.portals} aria-labelledby="problems-title">
      <div className={styles.problemsIntro}>
        <Reveal className={styles.portalsIntro}>
          <p className={styles.eyebrow}>Why teams switch</p>
          <h2 id="problems-title">
            Three problems.
            <br />
            <span>One browser that solves&nbsp;them.</span>
          </h2>
        </Reveal>
        <Reveal order={1}>
          <BotWall />
        </Reveal>
      </div>
      <Reveal>
        <ol className={styles.problems}>
          {PROBLEMS.map(({ n, title, problem, answer }) => (
            <li key={n}>
              <div className={styles.today}>
                <span className={styles.portalNumber}>{n}</span>
                <h3>{title}</h3>
                <p>{problem}</p>
              </div>
              <div className={styles.answer}>
                <span>With Oya</span>
                <p>{answer}</p>
              </div>
            </li>
          ))}
        </ol>
      </Reveal>
    </section>
  );
}
