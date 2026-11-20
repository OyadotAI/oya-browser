/** Public navigation keeps every destination and cleans up its compact-menu interactions. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { SiteHeader } from '@/app/_home/site-header';

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe('public header', () => {
  it('focuses the first destination on open and restores the trigger on Escape', () => {
    render(<SiteHeader />);
    const menu = screen.getByRole('button', { name: 'Menu' });
    fireEvent.click(menu);
    expect(menu.getAttribute('aria-expanded')).toBe('true');
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Product' }));
    fireEvent.keyDown(document.activeElement!, { key: 'Escape' });
    expect(menu.getAttribute('aria-expanded')).toBe('false');
    expect(document.activeElement).toBe(menu);
  });

  it('ignores Escape when all menus are closed and closes desktop Product on outside pointerdown', () => {
    render(
      <div>
        <SiteHeader />
        <footer>Outside footer</footer>
      </div>,
    );
    const docs = screen.getByRole('link', { name: 'Docs' });
    docs.focus();
    fireEvent.keyDown(docs, { key: 'Escape' });
    expect(document.activeElement).toBe(docs);
    const product = screen.getByRole('button', { name: 'Product' });
    fireEvent.click(product);
    expect(product.getAttribute('aria-expanded')).toBe('true');
    fireEvent.pointerDown(screen.getByText('Outside footer'));
    expect(product.getAttribute('aria-expanded')).toBe('false');
  });

  it('closes on destination and layout changes and removes its listener on unmount', () => {
    const add = vi.fn();
    const remove = vi.fn();
    vi.stubGlobal('matchMedia', () => ({ addEventListener: add, removeEventListener: remove }));
    const view = render(<SiteHeader />);
    const menu = screen.getByRole('button', { name: 'Menu' });
    fireEvent.click(menu);
    fireEvent.click(screen.getByRole('link', { name: 'Docs' }));
    expect(menu.getAttribute('aria-expanded')).toBe('false');
    fireEvent.click(menu);
    act(() => add.mock.calls[0][1]());
    expect(menu.getAttribute('aria-expanded')).toBe('false');
    view.unmount();
    expect(remove).toHaveBeenCalledWith('change', add.mock.calls[0][1]);
  });

  it('renders the same destinations as the home page header without requiring matchMedia', () => {
    vi.stubGlobal('matchMedia', undefined);
    render(<SiteHeader />);
    expect(screen.getAllByRole('navigation')).toHaveLength(1);
    expect(screen.getByRole('button', { name: 'Product' })).toBeTruthy();
    for (const [name, href] of [
      ['Workflows', '/#workflows'],
      ['Docs', '/docs'],
      ['Security', '/trust'],
    ])
      expect(screen.getByRole('link', { name }).getAttribute('href')).toBe(href);
  });
});
