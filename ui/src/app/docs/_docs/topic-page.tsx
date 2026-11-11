/**
 * A docs topic page (/docs/<slug>): one part of the docs on its own, with its
 * own title, description and address, so a search engine has a page per
 * subject to rank and an assistant has one to cite. /docs still shows every
 * part and the Markdown twins are still cut from it; this adds a way in, it
 * moves nothing. A server component, so the page file can export metadata and
 * the part's text is in the HTML a crawler reads.
 */
import type { PropsWithChildren } from 'react';
import type { Metadata } from 'next';
import Link from 'next/link';
import { ArrowLeft } from 'lucide-react';
import { answerMetadata } from '../../_answers/answer-page';
import { DocsHeader } from './docs-header';
import { DOC_TOPICS, docTopic, type DocTopicPage } from './pages';
import { TopicNav } from './topic-nav';

/** The part whose first heading has no rule above it (SectionHeading's `first`), so its lede brings its own gap. */
const NO_RULE_PART = 'getting-started';

/** The small links above the heading and in the pager. */
const QUIET_LINK = 'text-[12px] text-text-dim hover:text-text transition-colors';

/** The part a topic page is for. A page file naming a part with no topic is a mistake the build should stop on. */
function topicOf(slug: string): DocTopicPage {
  const page = docTopic(slug);
  if (!page) throw new Error(`No docs topic page for "${slug}"`);
  return page;
}

/**
 * A topic page's title, description, canonical address and social card, all its own: without
 * them it would inherit /docs's and every topic would claim to be the same page. The Markdown
 * twin is named as its alternate, so an agent reading the HTML finds the cheaper form.
 */
export function topicMetadata(slug: string): Metadata {
  const { topic } = topicOf(slug);
  const path = `/docs/${slug}`;
  return {
    ...answerMetadata({ title: topic.heading, lead: topic.description, path }),
    alternates: { canonical: path, types: { 'text/markdown': `${path}.md` } },
  };
}

/** Back to the full docs, and this part as Markdown. */
function TopRow({ slug }: { /** The part. */ slug: string }) {
  return (
    <div className="mb-5 flex items-center justify-between gap-4">
      <Link href="/docs" className={`inline-flex items-center gap-1.5 ${QUIET_LINK}`}>
        <ArrowLeft size={13} /> All docs
      </Link>
      <a href={`/docs/${slug}.md`} className={QUIET_LINK} title="This page as Markdown, for AI agents and LLMs">
        View as Markdown
      </a>
    </div>
  );
}

/** One pager card's props. */
interface PagerLinkProps {
  /** The topic it goes to. */
  page: DocTopicPage;
  /** "Previous" or "Next". */
  label: string;
  /** Extra classes, which place Next in the right column. */
  className?: string;
}

/** A card to a neighbouring topic. */
function PagerLink({ page, label, className = '' }: PagerLinkProps) {
  return (
    <Link
      href={`/docs/${page.slug}`}
      className={`group rounded-xl border border-border bg-bg-card/40 p-4 hover:border-accent/40 ${className}`}
    >
      <span className="block text-[12px] text-text-dim">{label}</span>
      <span className="mt-1 block text-[14.5px] font-medium text-text">{page.title}</span>
    </Link>
  );
}

/** Previous and Next: the only links that chain the topic pages together, so a crawler on one reaches them all. */
function Pager({ slug }: { /** The part. */ slug: string }) {
  const at = DOC_TOPICS.findIndex((page) => page.slug === slug);
  const previous = DOC_TOPICS[at - 1];
  const next = DOC_TOPICS[at + 1];
  return (
    <nav aria-label="Docs topics" className="mt-16 grid gap-3 grid-cols-1 sm:grid-cols-2">
      {previous && <PagerLink page={previous} label="Previous" />}
      {next && <PagerLink page={next} label="Next" className="sm:col-start-2 sm:text-right" />}
    </nav>
  );
}

/** The heading and first paragraph: what the page is, in the words a search result shows. */
function TopicIntro({ slug }: { /** The part. */ slug: string }) {
  const { topic } = topicOf(slug);
  return (
    <>
      <p className="eyebrow text-accent mb-3">Oya Browser docs</p>
      <h1 className="text-[30px] sm:text-[40px] leading-[1.1] font-medium tracking-[-.045em] text-text mb-5">
        {topic.heading}
      </h1>
      <p className={`docs-lede max-w-xl text-text-muted ${slug === NO_RULE_PART ? 'mb-10' : ''}`}>
        {topic.description}
      </p>
    </>
  );
}

/** TopicPage's props. */
interface Props extends PropsWithChildren {
  /** The part this page shows; its component is the child. */
  slug: string;
}

/** One part of the docs as its own page: header, intro, the part, and the way to the next. */
export function TopicPage({ slug, children }: Props) {
  return (
    <div className="docs-page min-h-screen bg-bg">
      <DocsHeader />
      <main className="min-w-0 px-5 pt-28 pb-24 sm:px-10 lg:pt-32">
        <div className="max-w-[800px] mx-auto">
          <TopRow slug={slug} />
          <TopicIntro slug={slug} />
          <TopicNav>
            <section data-doc-page={slug}>{children}</section>
          </TopicNav>
          <Pager slug={slug} />
        </div>
      </main>
    </div>
  );
}
