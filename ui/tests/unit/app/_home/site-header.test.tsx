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
    expect(document.activeElement).toBe(screen.getByRole('link', { name: 'Compare' }));
    fireEvent.keyDown(document.activeElement!, { key: 'Escape' });
    expect(menu.getAttribute('aria-expanded')).toBe('false');
    expect(document.activeElement).toBe(menu);
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

  it('renders one copy of every destination without requiring matchMedia', () => {
    vi.stubGlobal('matchMedia', undefined);
    render(<SiteHeader />);
    expect(screen.getAllByRole('navigation')).toHaveLength(1);
    for (const name of ['Compare', 'Research', 'Docs', 'Security', 'GitHub', 'Discord', 'Download', 'Open console']) {
      expect(screen.getAllByRole('link', { name })).toHaveLength(1);
    }
  });
});
