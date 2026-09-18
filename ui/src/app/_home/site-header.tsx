/**
 * The landing header.
 */
import Link from 'next/link';
import { OyaWordmark } from '@/components/oya-logo';
import { repository } from './content';
import styles from '../page.module.css';

/** Wordmark and main navigation. */
export function SiteHeader() {
  return (
    <header className={styles.header}>
      <OyaWordmark />
      <nav aria-label="Main navigation" className={styles.navigation}>
        <Link href="/docs">Docs</Link>
        <a href={repository}>GitHub</a>
        <Link href="/dashboard" className={styles.console}>
          Open console
        </Link>
      </nav>
    </header>
  );
}
