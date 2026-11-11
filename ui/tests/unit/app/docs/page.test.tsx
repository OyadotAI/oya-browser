/**
 * Unit tests for the docs page: every section's anchor is there, the search
 * finds and jumps, Escape clears it, "/" focuses it, and the sidebar and
 * in-page links navigate.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import DocsPage from '@/app/docs/page';
import { NAV_SCROLL_DELAY_MS, SEARCH_DEBOUNCE_MS } from '@/app/docs/_docs/constants';
import { DOC_TOPICS } from '@/app/docs/_docs/pages';

const scrolled = vi.fn();
beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  Element.prototype.scrollIntoView = function (this: Element) {
    scrolled(this.id);
  };
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  scrolled.mockClear();
  history.replaceState(null, '', '/');
});

/** Every section the sidebar links to. */
const SECTIONS = [
  'quickstart',
  'playbooks',
  'free-text-fields',
  'sdk',
  'cli',
  'download',
  'mcp-setup',
  'desktop-app',
  'cursor',
  'codex',
  'claude-ai',
  'try-it',
  'more-tools',
  'tabs',
  'personas',
  'self-hosting',
  'control-plane',
];

describe('DocsPage', () => {
  it('links each sidebar group heading to its topic page, which is how a crawler on /docs finds them', () => {
    render(<DocsPage />);
    const aside = document.querySelector('aside')!;
    const hrefs = [...aside.querySelectorAll('a[href^="/docs/"]')].map((a) => a.getAttribute('href'));
    expect(hrefs).toEqual(DOC_TOPICS.map(({ slug }) => `/docs/${slug}`));
  });

  it('leaves the heading of the part with no topic page as plain text', () => {
    render(<DocsPage />);
    const aside = document.querySelector('aside')!;
    expect([...aside.querySelectorAll('a')].some((a) => a.textContent === 'Architecture')).toBe(false);
    expect(aside.textContent).toContain('Architecture');
  });

  it('puts no topic-page link inside <main>, which the Markdown twins are cut from', () => {
    render(<DocsPage />);
    expect(document.querySelectorAll('main a[href^="/docs/"]:not([href$=".md"])')).toHaveLength(0);
  });

  it('renders every section anchor once', () => {
    render(<DocsPage />);
    for (const id of [...SECTIONS, 'rotation', 'onboarding', 'command-api', 'websocket']) {
      expect(document.querySelectorAll(`[id="${id}"]`)).toHaveLength(1);
    }
  });

  it('states the pool serves thirty-two tools and lists them without a key', () => {
    render(<DocsPage />);
    const setup = document.getElementById('mcp-setup')!.parentElement!;
    expect(setup.textContent).toContain('Thirty-two tools');
    expect(setup.textContent).toContain('without a key');
    expect(setup.textContent).toContain('http://127.0.0.1:47321/mcp');
  });

  it('shows three prompts to try, each with its expected result', () => {
    render(<DocsPage />);
    const tryIt = document.getElementById('try-it')!;
    const section = tryIt.parentElement!;
    const prompts = [...section.querySelectorAll('pre')].filter(
      (pre) => tryIt.compareDocumentPosition(pre) & Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(prompts).toHaveLength(3);
    for (const pre of prompts) expect(pre.querySelector('code')?.getAttribute('data-language')).toBe('plain');
    expect(prompts[0].textContent).toContain('instead of Chrome');
    expect(prompts[2].textContent).toContain('hand the tab back');
    expect(section.textContent).toContain('Expected: all 32 tools listed without signing in');
  });

  it('opens on the quickstart, ahead of the architecture', () => {
    render(<DocsPage />);
    const ids = [...document.querySelectorAll('main h2[id]')].map((h) => h.id);
    expect(ids[0]).toBe('quickstart');
    expect(ids.indexOf('playbooks')).toBeLessThan(ids.indexOf('control-plane'));
  });

  it('searches after a pause and jumps to a result', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<DocsPage />);
    await user.type(screen.getByRole('textbox', { name: 'Search documentation' }), 'MCP Setup');
    expect(screen.queryByRole('button', { name: 'MCP Setup' })).toBeNull();
    await act(() => vi.advanceTimersByTimeAsync(SEARCH_DEBOUNCE_MS));
    await user.click(screen.getByRole('button', { name: 'MCP Setup' }));
    expect(scrolled).toHaveBeenCalledWith('mcp-setup');
    expect((screen.getByRole('textbox') as HTMLInputElement).value).toBe('');
  });

  it('says so when nothing matches, and Escape clears and leaves the box', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<DocsPage />);
    const box = screen.getByRole('textbox', { name: 'Search documentation' });
    await user.type(box, 'zzzz');
    await act(() => vi.advanceTimersByTimeAsync(SEARCH_DEBOUNCE_MS));
    expect(screen.getByText('No results')).toBeTruthy();
    await user.keyboard('{Escape}');
    expect((box as HTMLInputElement).value).toBe('');
    expect(screen.queryByText('No results')).toBeNull();
    expect(document.activeElement).not.toBe(box);
  });

  it('a sidebar link marks its section, updates the URL and scrolls there', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<DocsPage />);
    const link = screen.getAllByRole('link', { name: 'Quickstart' })[0];
    await user.click(link);
    expect(link.getAttribute('aria-current')).toBe('location');
    expect(window.location.hash).toBe('#quickstart');
    await act(() => vi.advanceTimersByTimeAsync(NAV_SCROLL_DELAY_MS));
    expect(scrolled).toHaveBeenCalledWith('quickstart');
  });

  it('the in-page onboarding link navigates too', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<DocsPage />);
    await user.click(screen.getByRole('button', { name: 'onboarding' }));
    expect(window.location.hash).toBe('#onboarding');
  });

  it('opens the mobile menu as a dialog that a link closes', async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(<DocsPage />);
    await user.click(screen.getByRole('button', { name: 'Open documentation menu' }));
    const dialog = screen.getByRole('dialog', { name: 'Documentation' });
    const inMenu = [...dialog.querySelectorAll('a')].find((a) => a.textContent === 'TypeScript SDK')!;
    await user.click(inMenu);
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
