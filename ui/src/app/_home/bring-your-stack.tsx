/**
 * "Already have a stack? Change one line.": the line a browser-use, Playwright, Browserbase or coding-agent user
 * changes to run on Oya. Most people arrive with an agent already written; this shows them the cost of trying Oya is
 * one line, not a rewrite. The lines live in stack-content.ts, which the docs share.
 */
import { STACK_HEADING, STACK_LEAD, STACK_LINES } from './stack-content';
import styles from './stack.module.css';

/** The home page section. */
export function BringYourStack() {
  return (
    <section className="section" id="stack" aria-labelledby="stack-h">
      <div className="wrap">
        <h2 id="stack-h">{STACK_HEADING}</h2>
        <p className={styles.lead}>{STACK_LEAD}</p>
        <ul className={styles.lines}>
          {STACK_LINES.map(({ id, who, lang, line, note }) => (
            <li key={id} className={styles.line}>
              <span className={styles.who}>
                {who}
                <span className={styles.lang}>{lang}</span>
              </span>
              <code className={styles.code}>{line}</code>
              <span className={styles.note}>{note}</span>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
