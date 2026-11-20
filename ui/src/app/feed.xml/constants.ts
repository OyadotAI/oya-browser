/** Named constants for the /feed.xml RSS 2.0 endpoint. */

/** Maximum number of dated CHANGELOG.md releases included in the feed alongside research posts. */
export const MAX_CHANGELOG_FEED_ITEMS = 20;

/** Maximum number of bullet items summarized per release description. */
export const MAX_RELEASE_SUMMARY_ITEMS = 3;

/** Public cache header for the static RSS feed. */
export const FEED_CACHE_CONTROL = 'public, max-age=300';

/** Content-Type header for RSS 2.0 XML responses. */
export const FEED_CONTENT_TYPE = 'application/rss+xml; charset=utf-8';
