/**
 * sitemap.xml for canonical HTML pages. Markdown twins canonicalize to HTML;
 * discovery files and the API viewer are omitted by policy and remain linked
 * through alternate links and llms.txt. No editorial modification dates are known.
 */
import type { MetadataRoute } from 'next';
import { SITE_URL } from '@/lib/site';
import { DOC_TOPICS } from './docs/_docs/pages';
import { ANSWER_PAGES } from './_answers/content';
import { RESEARCH_POSTS } from './research/posts';

/** One canonical HTML page in the sitemap. */
interface Entry {
  /** Site-relative path. */
  path: string;
  /** How often the page changes. */
  changeFrequency: 'weekly' | 'monthly';
  /** Relative priority within the site. */
  priority: number;
}

/** Named sitemap priorities to avoid scattered numeric literals. */
const PRIORITY = { HOME: 1, CORE: 0.9, SECTION: 0.7, DETAIL: 0.6, LEGAL: 0.4 } as const;

/** Canonical HTML pages without experiment or vendor-check dates presented as editorial updates. */
const PAGES: Entry[] = [
  { path: '/', changeFrequency: 'weekly', priority: PRIORITY.HOME },
  { path: '/pricing', changeFrequency: 'weekly', priority: PRIORITY.CORE },
  { path: '/docs', changeFrequency: 'weekly', priority: PRIORITY.CORE },
  ...DOC_TOPICS.map(({ slug }): Entry => ({
    path: `/docs/${slug}`,
    changeFrequency: 'weekly',
    priority: PRIORITY.SECTION,
  })),
  { path: '/release-notes', changeFrequency: 'weekly', priority: PRIORITY.SECTION },
  { path: '/trust', changeFrequency: 'monthly', priority: PRIORITY.DETAIL },
  ...ANSWER_PAGES.map(({ path }): Entry => ({
    path,
    changeFrequency: 'monthly',
    priority: PRIORITY.SECTION,
  })),
  { path: '/vs', changeFrequency: 'monthly', priority: PRIORITY.SECTION },
  { path: '/research', changeFrequency: 'monthly', priority: PRIORITY.SECTION },
  ...RESEARCH_POSTS.map(({ path }): Entry => ({
    path,
    changeFrequency: 'monthly',
    priority: PRIORITY.DETAIL,
  })),
  { path: '/privacy', changeFrequency: 'monthly', priority: PRIORITY.LEGAL },
  { path: '/terms', changeFrequency: 'monthly', priority: PRIORITY.LEGAL },
];

/** Converts one sitemap Entry into a Next.js MetadataRoute.Sitemap item. */
const toSitemapItem = ({ path, changeFrequency, priority }: Entry) => ({
  url: `${SITE_URL}${path}`,
  changeFrequency,
  priority,
});

/** Deterministic sitemap of canonical HTML pages. */
export default function sitemap(): MetadataRoute.Sitemap {
  return PAGES.map(toSitemapItem);
}
