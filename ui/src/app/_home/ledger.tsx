/**
 * The audit strip under the hero replay: each line the playbook runs writes its row,
 * and once the last one lands the chain reads as verified. Every row is always in the
 * markup, shown or hidden by CSS, so the strip never changes height while it plays and
 * readers without script (or with reduced motion) see the whole finished trail.
 */
import { CheckCircle2, Link2 } from 'lucide-react';
import type { ReplayTask } from './content';
import styles from './ledger.module.css';

/** Props for the ledger: the task and the line running now. */
interface LedgerProps {
  /** The task being replayed. */
  task: ReplayTask;
  /** The index of the running line, or the line count once done. */
  step: number;
}

/** The audit rows written so far, and the verified mark at the end. */
export function Ledger({ task, step }: LedgerProps) {
  const done = step === task.code.length;
  return (
    <div className={styles.ledger}>
      <p className={styles.head}>
        <span>audit trail</span>
        <span className={styles.verified} data-shown={done}>
          <CheckCircle2 size={13} aria-hidden="true" /> Chain verified
        </span>
      </p>
      <ol className={styles.rows}>
        {task.ledger.map((row, i) => (
          <li key={row.hash} data-shown={i < step}>
            <time>{row.time}</time>
            <span className={styles.action}>{row.action}</span>
            <span className={styles.hash}>
              <Link2 size={11} aria-hidden="true" /> {row.hash}
            </span>
          </li>
        ))}
      </ol>
    </div>
  );
}
