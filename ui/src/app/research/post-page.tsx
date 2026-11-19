/**
 * The layout every research post shares: the date and setup first, then the method, the raw table
 * and the caveats, in the release notes' column style with the trust center's table. Numbers come
 * only from the post's data, which a test pins to its source document.
 */
import { Reveal } from '@/components/ui/reveal';
import type { Metadata } from 'next';
import styles from '../page.module.css';
import notes from '../release-notes/release-notes.module.css';
import trust from '../trust/trust.module.css';
import research from './research.module.css';
import { SiteHeader } from '../_home/site-header';
import { SiteFooter } from '../_home/site-footer';
import type { ResearchPost, ResearchTable } from './posts';

/** The post's title, its summary as the description, and its canonical address. */
export function postMetadata({ title, summary, path }: ResearchPost): Metadata {
  return { title, description: summary, alternates: { canonical: path }, openGraph: { type: 'article', url: path } };
}

/** The raw table, scrolling inside its own frame so a narrow screen never scrolls the page sideways. */
function Table({ caption, columns, rows }: ResearchTable) {
  return (
    <div className={research.scroll} tabIndex={0} role="region" aria-label={caption}>
      <table className={trust.table}>
        <caption className={research.caption}>{caption}</caption>
        <thead>
          <tr>
            {columns.map((column) => (
              <th key={column} scope="col">
                {column}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row[0]}>
              {row.map((cell, i) => (
                <td key={i}>{cell}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** The method, the table and the caveats. */
function Body({ post }: { /** The post. */ post: ResearchPost }) {
  return (
    <section className={notes.release}>
      <div className={notes.group}>
        <h3>Method</h3>
        <p>{post.method}</p>
      </div>
      <Table {...post.table} />
      <div className={notes.group}>
        <h3>Reading the numbers</h3>
        <ul>
          {post.notes.map((note) => (
            <li key={note}>{note}</li>
          ))}
        </ul>
      </div>
    </section>
  );
}

/** A research post page. */
export function PostPage({ post }: { /** What the page shows. */ post: ResearchPost }) {
  return (
    <div className={styles.page}>
      <SiteHeader />
      <div className={styles.container}>
        <main id="top" tabIndex={-1} className={notes.main}>
          <Reveal preset="public">
            <h1>{post.title}</h1>
            <p className={notes.lead}>
              Run {post.date} · {post.setup}
            </p>
          </Reveal>
          <Body post={post} />
        </main>
      </div>
      <SiteFooter />
    </div>
  );
}
