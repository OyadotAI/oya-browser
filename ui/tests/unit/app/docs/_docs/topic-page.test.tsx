/**
 * Unit tests for the docs topic pages (/docs/<slug>), through what a visitor and a crawler get:
 * the topic's own heading, description and address, its one part in the server's HTML, the way
 * back to all docs, its Markdown twin, and its neighbours.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import type { ComponentType } from 'react';
import type { Metadata } from 'next';
import { renderToStaticMarkup } from 'react-dom/server';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import GetStarted, { metadata as getStartedMetadata } from '@/app/docs/getting-started/page';
import Playbooks, { metadata as playbooksMetadata } from '@/app/docs/playbooks/page';
import Sdk, { metadata as sdkMetadata } from '@/app/docs/sdk/page';
import Cli, { metadata as cliMetadata } from '@/app/docs/cli/page';
import Mcp, { metadata as mcpMetadata } from '@/app/docs/mcp/page';
import McpTools, { metadata as mcpToolsMetadata } from '@/app/docs/mcp-tools/page';
import Identity, { metadata as identityMetadata } from '@/app/docs/identity/page';
import Anonymity, { metadata as anonymityMetadata } from '@/app/docs/anonymity/page';
import SelfHosting, { metadata as selfHostingMetadata } from '@/app/docs/self-hosting/page';
import Dashboard, { metadata as dashboardMetadata } from '@/app/docs/dashboard/page';
import Api, { metadata as apiMetadata } from '@/app/docs/api/page';
import { DOC_TOPICS, docTopic } from '@/app/docs/_docs/pages';
import { topicMetadata } from '@/app/docs/_docs/topic-page';

const push = vi.fn();
vi.mock('next/navigation', () => ({ useRouter: () => ({ push }) }));

afterEach(() => {
  cleanup();
  push.mockClear();
});

/** One topic page under test. */
interface Row {
  /** Its part. */
  slug: string;
  /** Its page component. */
  Page: ComponentType;
  /** The metadata its file exports. */
  metadata: Metadata;
  /** The id of a heading inside its part. */
  anchor: string;
}

const PAGES: Row[] = [
  { slug: 'getting-started', Page: GetStarted, metadata: getStartedMetadata, anchor: 'quickstart' },
  { slug: 'playbooks', Page: Playbooks, metadata: playbooksMetadata, anchor: 'playbooks' },
  { slug: 'sdk', Page: Sdk, metadata: sdkMetadata, anchor: 'sdk' },
  { slug: 'cli', Page: Cli, metadata: cliMetadata, anchor: 'cli' },
  { slug: 'mcp', Page: Mcp, metadata: mcpMetadata, anchor: 'mcp-setup' },
  { slug: 'mcp-tools', Page: McpTools, metadata: mcpToolsMetadata, anchor: 'analyze_page' },
  { slug: 'identity', Page: Identity, metadata: identityMetadata, anchor: 'personas' },
  { slug: 'anonymity', Page: Anonymity, metadata: anonymityMetadata, anchor: 'anonymity' },
  { slug: 'self-hosting', Page: SelfHosting, metadata: selfHostingMetadata, anchor: 'self-hosting' },
  { slug: 'dashboard', Page: Dashboard, metadata: dashboardMetadata, anchor: 'dashboard-overview' },
  { slug: 'api', Page: Api, metadata: apiMetadata, anchor: 'rest-api' },
];

/** The topic a row is for. */
const topicOf = (slug: string) => docTopic(slug)!;

/** The pager's link with this label, if the page has one. */
function pagerHref(label: string): string | null {
  const nav = screen.getByRole('navigation', { name: 'Docs topics' });
  const link = [...nav.querySelectorAll('a')].find((a) => a.textContent?.startsWith(label));
  return link?.getAttribute('href') ?? null;
}

describe.each(PAGES)('/docs/$slug', ({ slug, Page, metadata, anchor }) => {
  const { topic } = topicOf(slug);
  const at = DOC_TOPICS.findIndex((page) => page.slug === slug);

  it('opens with the topic heading and its description', () => {
    render(<Page />);
    expect(screen.getByRole('heading', { level: 1 }).textContent).toBe(topic.heading);
    expect(screen.getByText(topic.description)).toBeTruthy();
  });

  it('shows that one part and no other', () => {
    render(<Page />);
    const parts = [...document.querySelectorAll('section[data-doc-page]')].map((el) =>
      el.getAttribute('data-doc-page'),
    );
    expect(parts).toEqual([slug]);
  });

  it("has the part's text in the server's HTML, with its heading ids unchanged", () => {
    expect(renderToStaticMarkup(<Page />)).toContain(`id="${anchor}"`);
  });

  it('links back to all docs and to its Markdown twin', () => {
    render(<Page />);
    expect(screen.getByRole('link', { name: 'All docs' }).getAttribute('href')).toBe('/docs');
    expect(screen.getByRole('link', { name: 'View as Markdown' }).getAttribute('href')).toBe(`/docs/${slug}.md`);
  });

  it('links to the topics either side of it, and to nothing where there is none', () => {
    render(<Page />);
    const neighbour = (offset: number) => (DOC_TOPICS[at + offset] ? `/docs/${DOC_TOPICS[at + offset].slug}` : null);
    expect(pagerHref('Previous')).toBe(neighbour(-1));
    expect(pagerHref('Next')).toBe(neighbour(1));
  });

  it('has no menu button, since it has no menu', () => {
    render(<Page />);
    expect(screen.queryByRole('button', { name: 'Open documentation menu' })).toBeNull();
  });

  it('has its own title, description, canonical address and social card', () => {
    expect(metadata).toMatchObject({ title: topic.heading, description: topic.description });
    expect(metadata.alternates?.canonical).toBe(`/docs/${slug}`);
    expect(metadata.openGraph).toMatchObject({ title: topic.heading, url: `/docs/${slug}` });
  });

  it('names its Markdown twin as its alternate', () => {
    expect(metadata.alternates?.types).toEqual({ 'text/markdown': `/docs/${slug}.md` });
  });
});

describe('the topic page list', () => {
  it('has a test row for every topic, so a new page cannot skip these rules', () => {
    expect(PAGES.map((page) => page.slug)).toEqual(DOC_TOPICS.map((page) => page.slug));
  });

  it('never pages through the part that has no page of its own', () => {
    render(<SelfHosting />);
    expect(pagerHref('Next')).toBe('/docs/dashboard');
  });

  it('refuses to build a page for a part with no topic', () => {
    expect(() => topicMetadata('control-plane')).toThrow('No docs topic page for "control-plane"');
  });
});

describe('an in-text jump on a topic page', () => {
  it('goes to the full docs when its target is on another page', async () => {
    render(<GetStarted />);
    await userEvent.click(screen.getByRole('button', { name: 'Playbooks' }));
    expect(push).toHaveBeenCalledWith('/docs#playbooks');
  });
});
