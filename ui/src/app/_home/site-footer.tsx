/**
 * The landing footer.
 */
import Link from 'next/link';
import { OyaWordmark } from '@/components/oya-logo';
import { COMPLIANCE_LINE, discord, foundersCall, repository } from './content';
import { ANSWER_PAGES } from '../_answers/content';
import styles from '../page.module.css';

/** Wordmark and footer links. */
export function SiteFooter() {
  return (
    <footer className={styles.footer}>
      <OyaWordmark />
      <Link href="/trust" className={styles.complianceLine}>
        {COMPLIANCE_LINE}
      </Link>
      <nav aria-label="Footer navigation">
        <a href={`${repository}/tree/main/packages/sdk`}>SDK</a>
        <Link href="/docs#download">Desktop</Link>
        <a href={`${repository}/blob/main/docs/self-hosting.md`}>Self-host</a>
        {ANSWER_PAGES.map(({ path, label }) => (
          <Link key={path} href={path}>
            {label}
          </Link>
        ))}
        <Link href="/release-notes">Release notes</Link>
        <Link href="/trust">Trust</Link>
        <Link href="/privacy">Privacy</Link>
        <Link href="/terms">Terms</Link>
        <a href={repository}>GitHub</a>
        <a href={discord}>Discord</a>
        <a href={foundersCall}>Talk to Founders</a>
      </nav>
    </footer>
  );
}
