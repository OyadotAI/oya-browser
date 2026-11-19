/**
 * "Already have a stack? Change one line.": the line a browser-use, Playwright, Browserbase or coding-agent user
 * changes to run on Oya. Most people arrive with an agent already written; this shows them the cost of trying Oya is
 * one line, not a rewrite. The lines live in stack-content.ts, which the docs share.
 */
import { Reveal } from '@/components/ui/reveal';
import { ChevronDown } from 'lucide-react';
import { STACK_LINES } from './stack-content';
import styles from './stack.module.css';

/** The home page section. */
export function BringYourStack() {
  return (
    <section className="section" id="stack" aria-labelledby="stack-h">
      <Reveal className="wrap">
        <h2 id="stack-h">Call the saved playbook from your own tools.</h2>
        <p className={styles.lead}>Use MCP, the SDK or Playwright to run the playbook with new inputs.</p>
        <details className={styles.examples}>
          <summary>
            View connection examples
            <ChevronDown size={18} aria-hidden="true" />
          </summary>
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
        </details>
      </Reveal>
    </section>
  );
}
