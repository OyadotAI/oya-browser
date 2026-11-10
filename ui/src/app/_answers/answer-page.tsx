/**
 * The layout every answer page shares: the answer first, then sections, then questions that are
 * always visible (not collapsed), so a crawler and an assistant read every answer as plain text.
 * The same questions go out as FAQPage structured data. Styled as the trust center is.
 */
import type { Metadata } from 'next';
import styles from '../page.module.css';
import trust from '../trust/trust.module.css';
import { SiteHeader } from '../_home/site-header';
import { SiteFooter } from '../_home/site-footer';
import { faqJsonLd, type QA } from '@/lib/site';
import type { AnswerPageContent, AnswerSection } from './content';

/** The page's title, its answer as the description, and its canonical address. */
export function answerMetadata({ title, lead, path }: AnswerPageContent): Metadata {
  return { title, description: lead, alternates: { canonical: path } };
}

/** Structured data as script text: `<` escaped, so no string in it can close the script tag. */
const jsonLd = (data: object): string => JSON.stringify(data).replace(/</g, '\\u003c');

/** One section: heading, paragraph, and its list when it has one. */
function Section({ heading, body, items }: AnswerSection) {
  return (
    <section className={trust.section}>
      <h2>{heading}</h2>
      <p>{body}</p>
      {items && (
        <ul>
          {items.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** The questions, each answer shown. */
function Questions({ faq }: { /** The questions and answers. */ faq: QA[] }) {
  return (
    <section className={trust.section} aria-labelledby="questions">
      <h2 id="questions">Questions</h2>
      {faq.map(({ q, a }) => (
        <div key={q}>
          <h3>{q}</h3>
          <p>{a}</p>
        </div>
      ))}
    </section>
  );
}

/** An answer page. The structured data is a data block, never run, so it needs no CSP nonce. */
export function AnswerPage({ content }: { /** What the page says. */ content: AnswerPageContent }) {
  return (
    <div className={styles.page}>
      <div className={styles.container}>
        <SiteHeader />
        <main className={trust.main}>
          <h1>{content.title}</h1>
          <p className={trust.lead}>{content.lead}</p>
          {content.sections.map((section) => (
            <Section key={section.heading} {...section} />
          ))}
          <Questions faq={content.faq} />
        </main>
        <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: jsonLd(faqJsonLd(content.faq)) }} />
        <SiteFooter />
      </div>
    </div>
  );
}
