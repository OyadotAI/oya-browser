/**
 * The strip of supported browser providers.
 */
import styles from '../page.module.css';

/** The browser providers Oya drives. */
export function Providers() {
  return (
    <section className={styles.providers} aria-label="Supported browser providers">
      <p>Your browser. Your choice.</p>
      <ul>
        {['Oya Cloud', 'Browserbase', 'Browser Use', 'Steel', 'Anchor', 'Your own Chrome'].map((provider) => (
          <li key={provider}>{provider}</li>
        ))}
      </ul>
    </section>
  );
}
