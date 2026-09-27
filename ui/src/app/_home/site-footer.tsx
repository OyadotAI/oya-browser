/**
 * The landing footer.
 */
import Link from 'next/link';
import { OyaWordmark } from '@/components/oya-logo';
import { discord, foundersCall, repository } from './content';
import styles from '../page.module.css';

/** Wordmark and footer links. */
export function SiteFooter() {
  return (
    <footer className={styles.footer}>
      <OyaWordmark />
      <nav aria-label="Footer navigation">
        <a href={`${repository}/tree/main/packages/sdk`}>SDK</a>
        <Link href="/docs#download">Desktop</Link>
        <a href={`${repository}/blob/main/docs/self-hosting.md`}>Self-host</a>
        <Link href="/release-notes">Release notes</Link>
        <a href={repository}>GitHub</a>
        <a href={discord}>Discord</a>
        <a href={foundersCall}>Talk to the founders</a>
      </nav>
    </footer>
  );
}
