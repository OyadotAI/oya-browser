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
const SECTIONS = ['control-plane', 'quickstart', 'sdk', 'cli', 'download', 'mcp-setup', 'cursor', 'tabs', 'personas'];

describe('DocsPage', () => {
  it('renders every section anchor once', () => {
    render(<DocsPage />);
    for (const id of [...SECTIONS, 'rotation', 'onboarding', 'command-api', 'websocket']) {
      expect(document.querySelectorAll(`[id="${id}"]`)).toHaveLength(1);
    }
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
    const inMenu = [...dialog.querySelectorAll('a')].find((a) => a.textContent === 'SDK')!;
    await user.click(inMenu);
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
