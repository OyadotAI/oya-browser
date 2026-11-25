/**
 * Static RSS 2.0 feed at /feed.xml: baked at build time so standalone containers
 * do not need CHANGELOG.md on disk at runtime.
 */
import { readFileSync } from 'node:fs';
import { repoFile } from '@/lib/repo-file';
import { RESEARCH_POSTS } from '../research/posts';
import { FEED_CACHE_CONTROL, FEED_CONTENT_TYPE } from './constants';
import { buildFeedXml } from './feed';

/** Baked once at build time: CHANGELOG.md is copied into the build stage only. */
export const dynamic = 'force-static';

/** Repository-root changelog path. */
const CHANGELOG = repoFile('CHANGELOG.md');

/** Serves the RSS 2.0 XML feed of research posts and recent releases. */
export function GET(): Response {
  const xml = buildFeedXml(readFileSync(CHANGELOG, 'utf8'), RESEARCH_POSTS);
  return new Response(xml, {
    headers: { 'content-type': FEED_CONTENT_TYPE, 'cache-control': FEED_CACHE_CONTROL },
  });
}
