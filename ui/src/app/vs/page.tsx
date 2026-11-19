/** /vs: the index of comparison pages, so a buyer weighing Oya against another vendor finds them in one click. */
import { Reveal } from '@/components/ui/reveal';
import type { Metadata } from 'next';
import Link from 'next/link';
import styles from '../page.module.css';
import notes from '../release-notes/release-notes.module.css';
import { SiteHeader } from '../_home/site-header';
import { SiteFooter } from '../_home/site-footer';
import { ANSWER_PAGES } from '../_answers/content';

/** The page's title and description for search and link previews. */
export const metadata: Metadata = {
  title: 'Compare Oya Browser',
  description: 'How Oya Browser differs from other browsers for AI agents: Browserbase, Steel, Anchor and Browser Use.',
  alternates: { canonical: '/vs' },
};

/** The comparison pages, read from the answer pages so a new /vs page is listed without touching this file. */
const COMPARISONS = ANSWER_PAGES.filter(({ path }) => path.startsWith('/vs/'));

/** A lead's first statement, past the question it opens with, so each entry stays one line of answer. */
const firstSentence = (text: string): string => text.replace(/^[^.?]*\?\s*/, '').split(/(?<=\.)\s/)[0];

/** The comparison index. */
export default function Compare() {
  return (
    <div className={styles.page}>
      <SiteHeader />
      <div className={styles.container}>
        <main id="top" tabIndex={-1} className={notes.main}>
          <Reveal preset="public">
            <h1>Compare</h1>
            <p className={notes.lead}>How Oya Browser differs from other browsers for AI agents.</p>
          </Reveal>
          {COMPARISONS.map(({ path, title, lead, hero }) => (
            <section key={path} className={notes.release}>
              <h2>
                <Link href={path}>{hero?.headline ?? title}</Link>
              </h2>
              <p>{hero?.paragraphs[0] ?? firstSentence(lead)}</p>
            </section>
          ))}
        </main>
      </div>
      <SiteFooter />
    </div>
  );
}
