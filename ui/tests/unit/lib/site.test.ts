/** Public canonicals retain machine-readable discovery links when Next replaces nested metadata. */
import { expect, it } from 'vitest';
import { metadata as pricing } from '@/app/pricing/page';
import { metadata as coding } from '@/app/browser-for-claude-code/page';
import { metadata as docs } from '@/app/docs/layout';
import { metadata as mcp } from '@/app/docs/mcp/page';
import { metadata as reference } from '@/app/docs/api/reference/page';
import { metadata as research } from '@/app/research/page';
import { metadata as replay } from '@/app/research/replay/page';
import { metadata as compare } from '@/app/vs/page';
import { metadata as trust } from '@/app/trust/page';
import { metadata as privacy } from '@/app/privacy/page';
import { metadata as terms } from '@/app/terms/page';
import { metadata as releases } from '@/app/release-notes/page';

it.each([
  { metadata: pricing, canonical: '/pricing' },
  { metadata: coding, canonical: '/browser-for-claude-code' },
  { metadata: docs, canonical: '/docs' },
  { metadata: mcp, canonical: '/docs/mcp' },
  { metadata: reference, canonical: '/docs/api/reference' },
  { metadata: research, canonical: '/research' },
  { metadata: replay, canonical: '/research/replay' },
  { metadata: compare, canonical: '/vs' },
  { metadata: trust, canonical: '/trust' },
  { metadata: privacy, canonical: '/privacy' },
  { metadata: terms, canonical: '/terms' },
  { metadata: releases, canonical: '/release-notes' },
])('keeps agent, API and RSS discovery on $canonical', ({ metadata, canonical }) => {
  expect(metadata.alternates).toMatchObject({
    canonical,
    types: { 'text/plain': '/llms.txt', 'application/json': '/openapi.json', 'application/rss+xml': '/feed.xml' },
  });
});
