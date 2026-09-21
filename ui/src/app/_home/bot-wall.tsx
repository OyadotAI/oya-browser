/**
 * The hero figure: one portal, reached two ways. A headless browser driven
 * over the debugging protocol is turned away at the bot wall; the Oya browser
 * signs in and comes back with the answer.
 */
import { Ban, Check, Lock } from 'lucide-react';
import { BOT_WALL_LANES, BOT_WALL_ICON } from './content';
import styles from './bot-wall.module.css';

/** The icon each lane ends with. */
const RESULT_ICON = { blocked: Ban, through: Check };

/** One attempt at the portal: who is knocking, the trip, and what came back. */
function Lane({ lane }: { /** The lane to draw. */ lane: (typeof BOT_WALL_LANES)[number] }) {
  const Icon = RESULT_ICON[lane.kind];
  return (
    <div className={`${styles.lane} ${styles[lane.kind]}`}>
      <p className={styles.label}>
        <span className={styles.who}>{lane.who}</span>
        <span>{lane.how}</span>
      </p>
      <div className={styles.track}>
        <span className={styles.rail} />
        {lane.kind === 'blocked' && <span className={styles.wall} />}
        <span className={styles.dot} />
      </div>
      <div className={styles.outcome}>
        <span className={styles.result}>
          <Icon size={13} aria-hidden="true" /> {lane.result}
        </span>
      </div>
    </div>
  );
}

/** Both attempts, and the portal they both aim at. */
export function BotWall() {
  return (
    <figure
      className={styles.figure}
      aria-label="Two attempts at the same payer portal. A headless browser driven over the debugging protocol is blocked at the bot wall. The Oya browser signs in and returns the eligibility answer."
    >
      <div className={styles.grid}>
        <div>
          {BOT_WALL_LANES.map((lane) => (
            <Lane key={lane.kind} lane={lane} />
          ))}
        </div>
        <aside className={styles.portal}>
          <Lock size={BOT_WALL_ICON} strokeWidth={1.5} aria-hidden="true" />
          <strong>Payer portal</strong>
          <span>Bot wall, SSO, MFA</span>
        </aside>
      </div>
      <figcaption className={styles.caption}>
        Same portal, same task. The difference is where the automation runs.
      </figcaption>
    </figure>
  );
}
