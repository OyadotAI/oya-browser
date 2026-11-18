/**
 * The page's closing footer: a clean bordered CTA card with a standard primary button,
 * followed by the five-column directory and legal row. Carries the #download anchor.
 */
import { Reveal } from '@/components/ui/reveal';
import Link from 'next/link';
import { ANSWER_PAGES } from '../_answers/content';
import { OyaLogo } from '@/components/oya-logo';
import { browserDownloads } from '@/lib/browser-downloads';
import { discord, foundersCall, repository } from './content';
import { Arrow } from './arrow';
import { CLOSING_MARK_SIZE } from './constants';
import styles from './closing.module.css';

/** The directory's columns: a heading and its links. */
const COLUMNS: [string, [string, string][]][] = [
  [
    'Product',
    [
      ['Playbooks', '#playbooks'],
      ['Workflows', '#workflows'],
      ['Audit trail', '#audit'],
      ['Research', '/research'],
    ],
  ],
  [
    'Start',
    [
      ['Console', '/dashboard'],
      ['Docs', '/docs'],
      ['Compare', '/vs'],
      ['Book a demo', foundersCall],
    ],
  ],
  [
    'Company',
    [
      ['Security', '/trust'],
      ['Questions', '#faq'],
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

/** The answer pages, linked from the home page so crawlers reach them; the grid above is full, so they sit in the legal row. */
function AnswerLinks() {
  return (
    <nav aria-label="Answers">
      {ANSWER_PAGES.map(({ path, label }) => (
        <Link key={path} href={path}>
          {label}
        </Link>
      ))}
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
        <AnswerLinks />
      </div>
    </div>
  );
}

/** The closing footer. */
export function Closing() {
  return (
    <footer className={styles.footer} id="contact" aria-labelledby="close-h">
      <div className="closing__panel">
        <Reveal className="wrap closing__cta">
          <span className="closing__mark" aria-hidden="true">
            <OyaLogo size={CLOSING_MARK_SIZE} />
          </span>
          <div className="section-head">
            <h2 id="close-h">Scope your first workflow with the founders.</h2>
            <p className="t-lead">Bring your SOP or screen recording. Scope your first workflow with the founders.</p>
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
        </Reveal>
      </div>
      <div className="wrap">
        <Directory />
      </div>
    </footer>
  );
}
