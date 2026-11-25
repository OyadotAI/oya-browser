/** Reuses the app's readable font without loading global landing styles or extra typefaces. */
import styles from './landing.module.css';

/** Keeps the landing font declaration local, even when navigating to another route. */
export function Kit({ nonce }: { /** Allows the font declaration through the request's CSP. */ nonce?: string }) {
  return (
    <style nonce={nonce}>{`.${styles.page} {
      --font-text: var(--font-dm-sans), sans-serif;
      --font-mono: ui-monospace, 'SFMono-Regular', Menlo, Consolas, monospace;
    }`}</style>
  );
}
