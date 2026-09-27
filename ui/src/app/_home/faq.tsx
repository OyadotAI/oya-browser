/**
 * The questions a buyer asks before wiring Oya into a workflow, answered in a
 * sentence or two each. Native disclosure elements, so they open without script.
 */
import { Plus } from 'lucide-react';
import { FAQ } from './content';
import styles from '../page.module.css';

/** The FAQ, one disclosure per question. */
export function Faq() {
  return (
    <section className={styles.faq} aria-labelledby="faq-title">
      <div className={styles.portalsIntro}>
        <p className={styles.eyebrow}>Questions</p>
        <h2 id="faq-title">
          Before you wire it in.
          <br />
          <span>Straight answers.</span>
        </h2>
      </div>
      <div className={styles.faqList}>
        {FAQ.map(({ q, a }) => (
          <details key={q}>
            <summary>
              {q}
              <Plus size={16} aria-hidden="true" />
            </summary>
            <p>{a}</p>
          </details>
        ))}
      </div>
    </section>
  );
}
