/**
 * Unit tests for the theme toggle: it flips the page theme, remembers it,
 * and every toggle on the page follows.
 */
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ThemeToggle from '@/components/theme-toggle';

afterEach(() => {
  cleanup();
  delete document.documentElement.dataset.theme;
  localStorage.clear();
});

describe('ThemeToggle', () => {
  it('switches to light and back, remembering the choice, with every toggle in step', async () => {
    render(
      <>
        <ThemeToggle />
        <ThemeToggle />
      </>,
    );
    await userEvent.click(screen.getAllByRole('button', { name: 'Switch to light theme' })[0]);
    expect(document.documentElement.dataset.theme).toBe('light');
    expect(localStorage.getItem('oya_theme')).toBe('light');
    expect(screen.getAllByRole('button', { name: 'Switch to dark theme' })).toHaveLength(2);
    await userEvent.click(screen.getAllByRole('button', { name: 'Switch to dark theme' })[1]);
    expect(document.documentElement.dataset.theme).toBe('dark');
  });
});
