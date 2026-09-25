/**
 * sitemap.xml for the public pages.
 */
import type { MetadataRoute } from 'next';
import { SITE_URL } from '@/lib/site';

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
 * Public pages only, plus the files written for agents (answer engines cite
 * what they can find). /dashboard and /live/* are per-account and behind auth,
 * so listing them would just advertise redirects.
 */
const PAGES: Entry[] = [
  { path: '/', changeFrequency: 'weekly', priority: 1 },
  { path: '/docs', changeFrequency: 'weekly', priority: 0.9 },
  { path: '/llms.txt', changeFrequency: 'weekly', priority: 0.8 },
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
