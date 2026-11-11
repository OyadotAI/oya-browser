/**
 * sitemap.xml for the public pages.
 */
import type { MetadataRoute } from 'next';
import { SITE_URL } from '@/lib/site';
import { DOC_PAGES, DOC_TOPICS } from './docs/_docs/pages';
import { ANSWER_PAGES } from './_answers/content';

/** One page of the sitemap. */
interface Entry {
  /** Its path. */
  path: string;
  /** How often it changes. */
  changeFrequency: 'weekly' | 'monthly';
  /** How much it matters next to the others. */
  priority: number;
}

/**
 * Public pages only, each docs topic's own page, plus the files written for agents: llms.txt and the docs'
 * Markdown twins (answer engines cite what they can find). /dashboard and /live/* are per-account and behind auth,
 * so listing them would just advertise redirects.
 */
const PAGES: Entry[] = [
  { path: '/', changeFrequency: 'weekly', priority: 1 },
  { path: '/docs', changeFrequency: 'weekly', priority: 0.9 },
  ...DOC_TOPICS.map(({ slug }): Entry => ({ path: `/docs/${slug}`, changeFrequency: 'weekly', priority: 0.7 })),
  { path: '/docs.md', changeFrequency: 'weekly', priority: 0.8 },
  ...DOC_PAGES.map(({ slug }): Entry => ({ path: `/docs/${slug}.md`, changeFrequency: 'weekly', priority: 0.6 })),
  { path: '/release-notes', changeFrequency: 'weekly', priority: 0.7 },
  { path: '/trust', changeFrequency: 'monthly', priority: 0.6 },
  ...ANSWER_PAGES.map(({ path }): Entry => ({ path, changeFrequency: 'monthly', priority: 0.7 })),
  { path: '/privacy', changeFrequency: 'monthly', priority: 0.4 },
  { path: '/terms', changeFrequency: 'monthly', priority: 0.4 },
  { path: '/llms.txt', changeFrequency: 'weekly', priority: 0.8 },
  { path: '/llms-full.txt', changeFrequency: 'weekly', priority: 0.6 },
  { path: '/openapi.json', changeFrequency: 'weekly', priority: 0.5 },
  { path: '/signup', changeFrequency: 'monthly', priority: 0.5 },
  { path: '/login', changeFrequency: 'monthly', priority: 0.3 },
];

/** Every public page, stamped with now. */
export default function sitemap(): MetadataRoute.Sitemap {
  const lastModified = new Date();
  return PAGES.map(({ path, changeFrequency, priority }) => ({
    url: `${SITE_URL}${path}`,
    lastModified,
    changeFrequency,
    priority,
  }));
}
