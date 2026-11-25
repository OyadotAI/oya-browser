/**
 * Unit tests for public page meta descriptions flagged by search-engine audits:
 * verifies /release-notes, /privacy, /terms and /vs stay within 150-160 chars
 * with zero em-dashes.
 */
import { describe, it, expect } from 'vitest';
import { metadata as releaseNotesMetadata } from '@/app/release-notes/page';
import { metadata as privacyMetadata } from '@/app/privacy/page';
import { metadata as termsMetadata } from '@/app/terms/page';
import { metadata as vsMetadata } from '@/app/vs/page';

/** Minimum meta description length recommended by Bing Webmaster Tools. */
const MIN_DESCRIPTION_CHARS = 150;

/** Maximum meta description length before SERP truncation. */
const MAX_DESCRIPTION_CHARS = 160;

/** Public pages whose descriptions are pinned to the 150-160 character window. */
const MONITORED_PAGES = [
  { path: '/release-notes', metadata: releaseNotesMetadata },
  { path: '/privacy', metadata: privacyMetadata },
  { path: '/terms', metadata: termsMetadata },
  { path: '/vs', metadata: vsMetadata },
] as const;

describe('monitored public page meta descriptions', () => {
  it.each(MONITORED_PAGES)('$path emits a 150-160 char description with no em-dashes', ({ metadata }) => {
    const description = typeof metadata.description === 'string' ? metadata.description : '';
    expect(description.length).toBeGreaterThanOrEqual(MIN_DESCRIPTION_CHARS);
    expect(description.length).toBeLessThanOrEqual(MAX_DESCRIPTION_CHARS);
    expect(description).not.toContain('—');
  });
});
