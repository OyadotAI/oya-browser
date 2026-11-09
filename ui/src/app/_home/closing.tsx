/**
 * The page's closing footer: a clean bordered CTA card with a standard primary button,
 * followed by the five-column directory and legal row. Carries the #download anchor.
 */
import Link from 'next/link';
import { OyaLogo } from '@/components/oya-logo';
import { browserDownloads } from '@/lib/browser-downloads';
import { discord, foundersCall, repository } from './content';
import { Arrow } from './arrow';

/** The directory's columns: a heading and its links. */
const COLUMNS: [string, [string, string][]][] = [
  [
    'Product',
    [
      ['Playbooks', '#playbooks'],
      ['Workflows', '#workflows'],
      ['Audit trail', '#audit'],
    ],
  ],
  [
    'Start',
    [
      ['Console', '/dashboard'],
      ['Docs', '/docs'],
      ['Book a demo', foundersCall],
    ],
  ],
  [
    'Company',
    [
      ['Security', '/trust'],
      ['GitHub', repository],
      ['Discord', discord],
    ],
  ],
];

/** The desktop downloads, one link per platform. */
function Downloads() {
  return (
    <nav className="span-2 footer__col" id="download" aria-label="Browser downloads">
      <h3>Download</h3>
      <ul>
        {browserDownloads.map(({ platform, href }) => (
          <li key={platform}>
            <a href={href} download data-track="download_clicked" data-track-label={platform}>
              Oya Browser for {platform}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  );
}

/** The directory under the close: the mark and lede, the link columns, the downloads and the legal row. */
function Directory() {
  return (
    <div className="closing__links">
      <div className="grid">
        <div className="span-4 stack-4">
          <Link className="nav__logo" href="/">
            <OyaLogo size={24} /> Oya
          </Link>
          <p className="footer__lede">
            Browser automation for healthtech and RCM companies. Built by engineers from Google; AWS Partner, AWS
            Startups and Google for Startups.
          </p>
        </div>
        {COLUMNS.map(([head, links]) => (
          <div key={head} className="span-2 footer__col">
            <h3>{head}</h3>
            <ul>
              {links.map(([label, href]) => (
                <li key={label}>
                  <a href={href}>{label}</a>
                </li>
              ))}
            </ul>
          </div>
        ))}
        <Downloads />
      </div>
      <div className="footer__legal">
        <span>
          &copy; 2026 Oya &nbsp; <Link href="/privacy">Privacy</Link> &nbsp; <Link href="/terms">Terms</Link>
        </span>
      </div>
    </div>
  );
}

/** The closing footer. */
export function Closing() {
  return (
    <footer className="footer closing" id="contact" aria-labelledby="close-h">
      <div className="wrap">
        <div className="closing__cta">
          <div className="section-head">
            <h2 id="close-h">Bring us your first workflow.</h2>
            <p className="t-lead">Discuss it with the founders in a 30-minute call.</p>
          </div>
          <a
            className="btn btn--primary"
            href={foundersCall}
            data-track="cta_clicked"
            data-track-label="closing_founders"
          >
            Book a demo
            <Arrow />
          </a>
        </div>
        <Directory />
      </div>
    </footer>
  );
}
