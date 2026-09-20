/**
 * The landing section for developers: the portal example and its copy button.
 */
import CopyExample from '@/components/copy-example';
import SyntaxCode from '@/components/ui/syntax-code';
import { portalExample, repository } from './content';
import { ArrowRight, Braces, Fingerprint } from 'lucide-react';
import styles from '../page.module.css';

/** The code example: record once, replay with new inputs. */
export function Developers() {
  return (
    <section id="developers" className={styles.developers} aria-labelledby="example-title">
      <div className={styles.exampleIntro}>
        <Braces size={24} strokeWidth={1.5} aria-hidden="true" />
        <h2 id="example-title">
          A task today.
          <br />A playbook tomorrow.
        </h2>
        <p>Your model. Your inputs. Playwright code you can inspect and keep.</p>
        <a href={`${repository}/tree/main/packages/sdk#portal-automation-record-once-replay-with-new-inputs`}>
          Get the full example <ArrowRight size={15} aria-hidden="true" />
        </a>
        <div className={styles.profileNote}>
          <Fingerprint size={18} aria-hidden="true" />
          <span>Saved profiles keep your identity and login cookies across runs.</span>
        </div>
      </div>
      <div className={styles.example}>
        <div className={styles.codeHeader}>
          <span>
            portal.ts <span className={styles.runtime}>Node 24+</span>
          </span>
          <CopyExample code={portalExample} />
        </div>
        <pre tabIndex={0} aria-label="Portal automation TypeScript example">
          <SyntaxCode code={portalExample} language="typescript" />
        </pre>
        <p className={styles.codeNote}>Fictional inputs. Adapt the task to your test portal.</p>
      </div>
    </section>
  );
}
