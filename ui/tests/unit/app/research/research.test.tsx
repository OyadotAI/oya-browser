/**
 * Unit tests for /research, its posts and the /vs index: every number on a post appears in the
 * document it is copied from, the tables are readable by a screen reader, each page is in the
 * sitemap, and Compare and Research remain reachable through the site and home footers.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import Research from '@/app/research/page';
import ReplayResearch, { metadata as replayMetadata } from '@/app/research/replay/page';
import StealthResearch, { metadata as stealthMetadata } from '@/app/research/stealth/page';
import Compare from '@/app/vs/page';
import { RESEARCH_POSTS, REPLAY_POST, STEALTH_POST } from '@/app/research/posts';
import { ANSWER_PAGES } from '@/app/_answers/content';
import { SiteFooter } from '@/app/_home/site-footer';
import { Closing } from '@/app/_home/closing';
import { SITE_URL } from '@/lib/site';
import sitemap from '@/app/sitemap';

/** A source document as plain text: bold markers removed, so a bold cell matches its plain copy. */
const source = (file: string): string =>
  readFileSync(path.join(process.cwd(), '..', file), 'utf8').replace(/\*\*/g, '');

afterEach(() => cleanup());

describe.each(RESEARCH_POSTS)('$path', (post) => {
  it('quotes only cells and a date that appear in its source document', () => {
    const text = source(post.source);
    expect(text).toContain(post.date);
    for (const cell of post.table.rows.flat()) expect(text).toContain(cell);
  });

  it('has a method and the run date', () => {
    expect(post.method.length).toBeGreaterThan(0);
    expect(post.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('is in the sitemap', () => {
    expect(sitemap().map((e) => e.url)).toContain(`${SITE_URL}${post.path}`);
  });
});

describe('research post pages', () => {
  it.each([
    { Page: ReplayResearch, post: REPLAY_POST },
    { Page: StealthResearch, post: STEALTH_POST },
  ])('shows $post.title with a captioned table and column headers', ({ Page, post }) => {
    render(<Page />);
    expect(screen.getByRole('heading', { level: 1, name: post.title })).toBeTruthy();
    expect(screen.getByText(`Run ${post.date} · ${post.setup}`)).toBeTruthy();
    const table = screen.getByRole('table', { name: post.table.caption });
    expect(within(table).getAllByRole('columnheader')).toHaveLength(post.table.columns.length);
    expect(within(table).getAllByRole('columnheader')[0].getAttribute('scope')).toBe('col');
  });

  it('gives each post its own canonical address', () => {
    expect(replayMetadata.alternates?.canonical).toBe(REPLAY_POST.path);
    expect(stealthMetadata.alternates?.canonical).toBe(STEALTH_POST.path);
  });

  it('says the stealth run measured headless Chrome, not the desktop app', () => {
    expect(STEALTH_POST.method).toContain('not the desktop app');
  });
});

describe('/research', () => {
  it('links every post', () => {
    render(<Research />);
    for (const { title, path: href } of RESEARCH_POSTS) {
      expect(screen.getByRole('link', { name: title }).getAttribute('href')).toBe(href);
    }
  });
});

describe('/vs', () => {
  it('links every comparison page and nothing else', () => {
    render(<Compare />);
    const main = within(screen.getByRole('main'));
    const comparisons = ANSWER_PAGES.filter(({ path: p }) => p.startsWith('/vs/'));
    expect(main.getAllByRole('link')).toHaveLength(comparisons.length);
    for (const { title, hero, path: href } of comparisons) {
      expect(main.getByRole('link', { name: hero?.headline ?? title }).getAttribute('href')).toBe(href);
    }
  });

  it('shows the answer, not the question a lead opens with', () => {
    render(<Compare />);
    expect(screen.queryByText(/^Choosing a browser/)).toBeNull();
  });

  it('is in the sitemap with /research', () => {
    const urls = sitemap().map((e) => e.url);
    expect(urls).toContain(`${SITE_URL}/vs`);
    expect(urls).toContain(`${SITE_URL}/research`);
  });
});

describe('navigation', () => {
  it.each([
    ['the site footer', () => render(<SiteFooter />)],
    ['the home footer', () => render(<Closing />)],
  ])('links Compare and Research from %s', (_, show) => {
    show();
    expect(screen.getByRole('link', { name: 'Compare' }).getAttribute('href')).toBe('/vs');
    expect(screen.getByRole('link', { name: 'Research' }).getAttribute('href')).toBe('/research');
  });
});
