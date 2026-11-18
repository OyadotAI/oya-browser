/**
 * The layout the Privacy Policy and Terms of Service share: the site header, a
 * title with its effective date and lead, numbered sections, and the footer.
 * It reads in the trust center's column style.
 */
import { Reveal } from '@/components/ui/reveal';
import type { ReactNode } from 'react';
import styles from '../page.module.css';
import trust from '../trust/trust.module.css';
import legal from './legal.module.css';
import { SiteHeader } from '../_home/site-header';
import { SiteFooter } from '../_home/site-footer';

/** One section of a legal page. */
export interface LegalSection {
  /** Its heading, also its anchor. */
  heading: string;
  /** Its paragraphs and lists. */
  body: ReactNode;
}

/** What a legal page is made of. */
interface LegalPageProps {
  /** The page title. */
  title: string;
  /** When this version took effect, in words. */
  effective: string;
  /** The opening paragraph. */
  lead: ReactNode;
  /** The sections, in order. */
  sections: readonly LegalSection[];
}

/** A heading's anchor: lower case, words joined by hyphens. */
const anchor = (heading: string): string =>
  heading
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');

/** The numbered contents, so a long policy can be skimmed. */
function Contents({ sections }: Pick<LegalPageProps, 'sections'>) {
  return (
    <nav className={legal.contents} aria-label="Contents">
      <ol>
        {sections.map((s) => (
          <li key={s.heading}>
            <a href={`#${anchor(s.heading)}`}>{s.heading}</a>
          </li>
        ))}
      </ol>
    </nav>
  );
}

/** A Privacy Policy or Terms of Service page. */
export function LegalPage({ title, effective, lead, sections }: LegalPageProps) {
  return (
    <div className={styles.page}>
      <div className={styles.container}>
        <SiteHeader />
        <main className={trust.main}>
          <Reveal preset="public">
            <h1>{title}</h1>
            <p className={legal.effective}>Effective {effective}</p>
            <div className={trust.lead}>{lead}</div>
          </Reveal>
          <Contents sections={sections} />
          {sections.map((s, i) => (
            <section key={s.heading} id={anchor(s.heading)} className={`${trust.section} ${legal.section}`}>
              <h2>
                <span className={legal.number}>{i + 1}.</span> {s.heading}
              </h2>
              {s.body}
            </section>
          ))}
        </main>
        <SiteFooter />
      </div>
    </div>
  );
}
