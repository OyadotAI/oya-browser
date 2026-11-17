/** /research: the index of what we measured, each post with its run date and one-line result. */
import type { Metadata } from 'next';
import Link from 'next/link';
import styles from '../page.module.css';
import notes from '../release-notes/release-notes.module.css';
import research from './research.module.css';
import { SiteHeader } from '../_home/site-header';
import { SiteFooter } from '../_home/site-footer';
import { RESEARCH_POSTS } from './posts';

/** The page's title and description for search and link previews. */
export const metadata: Metadata = {
  title: 'Research',
  description: 'What we measured about Oya Browser, how, and the raw numbers.',
  alternates: { canonical: '/research' },
};

/** The research index. */
export default function Research() {
  return (
    <div className={styles.page}>
      <div className={styles.container}>
        <SiteHeader />
        <main className={notes.main}>
          <h1>Research</h1>
          <p className={notes.lead}>What we measured, how, and the raw numbers.</p>
          {RESEARCH_POSTS.map(({ path, title, date, summary }) => (
            <section key={path} className={notes.release}>
              <h2>
                <Link href={path}>{title}</Link>
              </h2>
              <p className={research.date}>Run {date}</p>
              <p>{summary}</p>
            </section>
          ))}
        </main>
        <SiteFooter />
      </div>
    </div>
  );
}
