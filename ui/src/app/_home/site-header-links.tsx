/** A single navigation tree keeps every public destination available on phones and desktops. */
import Link from 'next/link';
import { discord, repository } from './content';
import styles from '../page.module.css';

/** Stable analytics labels and destinations are retained while the visual layout changes. */
const LINKS = [
  { href: '/vs', label: 'Compare', track: 'compare' },
  { href: '/research', label: 'Research', track: 'research' },
  { href: '/docs', label: 'Docs', track: 'docs' },
  { href: '/trust', label: 'Security', track: 'security' },
  { href: repository, label: 'GitHub', track: 'github' },
  { href: discord, label: 'Discord', track: 'discord' },
  { href: '/#download', label: 'Download', track: 'download', className: styles.headerDownload },
  { href: '/dashboard', label: 'Open console', track: 'console', className: styles.console },
];

/** The disclosure changes visibility without duplicating links or their keyboard stops. */
export function SiteLinks({
  open,
  close,
}: {
  /** Whether the compact header menu is open. */ open: boolean;
  /** Closes the menu after selecting a destination. */ close: () => void;
}) {
  return (
    <nav id="main-links" aria-label="Main navigation" className={styles.navigation} data-open={open} onClick={close}>
      {LINKS.map(({ href, label, track, className }) => (
        <Link
          key={track}
          href={href}
          className={className}
          data-track="cta_clicked"
          data-track-label={`header_${track}`}
        >
          {label}
        </Link>
      ))}
    </nav>
  );
}
