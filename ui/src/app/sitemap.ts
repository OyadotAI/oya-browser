/**
 * sitemap.xml for the public pages.
 */
import type { MetadataRoute } from 'next';
import { SITE_URL } from '@/lib/site';

/**
 * Public pages only. /dashboard and /live/* are per-account and behind auth,
 * so listing them would just advertise redirects.
 */
export default function sitemap(): MetadataRoute.Sitemap {
  const now = new Date();
  return [
    { url: `${SITE_URL}/`, lastModified: now, changeFrequency: 'weekly', priority: 1 },
    { url: `${SITE_URL}/docs`, lastModified: now, changeFrequency: 'weekly', priority: 0.9 },
    { url: `${SITE_URL}/signup`, lastModified: now, changeFrequency: 'monthly', priority: 0.5 },
    { url: `${SITE_URL}/login`, lastModified: now, changeFrequency: 'monthly', priority: 0.3 },
  ];
}
