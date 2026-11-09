/**
 * Track everything, as the kit's split-text-left on the page's one surface band: what the
 * trail records, three facts a buyer can check, and the trail itself (agents and people on
 * one chain, every row linked by its hash) sitting in the frame. No moment here: the dealt
 * line-carries-colour covered the whole section with a blank card until the scroll reached it,
 * which read as a missing picture; the page's one moment is the statement's.
 */
import Link from 'next/link';
import type { CSSProperties } from 'react';
import { CheckCircle2, Link2 } from 'lucide-react';
import { AUDIT_FACTS, AUDIT_ROWS, COMPLIANCE_LINE } from './content';
import { SPLIT_RATIO } from './constants';
import styles from './audit.module.css';

/** Where the trail sits in the frame: at the corner, as wide as the frame allows, cut by the bottom. */
const PLACE = { '--ax': '56px', '--ay': '56px', right: '56px' } as CSSProperties;

/** The example trail: one row per action, each linked to the row before. */
function Trail() {
  return (
    <figure className={styles.trail} aria-label="An example audit trail for one eligibility check">
      <figcaption className={styles.trailHead}>
        <span>audit_events · session 4821</span>
        <span className={styles.verified}>
          <CheckCircle2 size={13} aria-hidden="true" /> Chain verified
        </span>
      </figcaption>
      <ol className={styles.rows}>
        {AUDIT_ROWS.map((row) => (
          <li key={row.hash} className={styles.row} data-person={row.actor.startsWith('person') || undefined}>
            <time>{row.time}</time>
            <span className={styles.actor}>{row.actor}</span>
            <span className={styles.action}>{row.action}</span>
            <span className={styles.hash}>
              <Link2 size={11} aria-hidden="true" /> {row.hash}
            </span>
          </li>
        ))}
      </ol>
    </figure>
  );
}

/** The audit section. */
export function Audit() {
  return (
    <section className="section ground-surface" id="audit" aria-labelledby="audit-h">
      <div className="wrap">
        <div className="grid grid--top">
          <div className="span-5">
            <h2 id="audit-h">Every action on the record</h2>
            <p className="t-lead">
              Each action an agent takes, and each time a person steps in, lands on one audit trail with who did it,
              their role and the key they used.
            </p>
            <ul className={styles.facts}>
              {AUDIT_FACTS.map((fact) => (
                <li key={fact}>{fact}</li>
              ))}
              <li>
                <Link href="/trust" data-track="cta_clicked" data-track-label="audit_trust">
                  {COMPLIANCE_LINE}
                </Link>
              </li>
            </ul>
            <Link className="link" href="/trust">
              Read the security page
            </Link>
          </div>
          <div className="frame frame--split span-6 start-7" style={SPLIT_RATIO}>
            <div className="stage stage--fade-br">
              <div className="at-corner to-edge" style={PLACE}>
                <Trail />
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
