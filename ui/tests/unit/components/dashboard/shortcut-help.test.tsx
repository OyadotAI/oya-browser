/**
 * Unit tests for the shortcut cheat sheet: shortcuts are listed under their
 * group.
 */
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import ShortcutHelp from '@/components/dashboard/shortcut-help';
import type { Shortcut } from '@/lib/shortcuts';

const shortcuts = [
  { keys: 'j', label: 'Next browser', group: 'Fleet' },
  { keys: 'g p', label: 'Go to personas', group: 'Navigate' },
] as unknown as Shortcut[];

describe('ShortcutHelp', () => {
  afterEach(cleanup);

  it('lists each shortcut under its own group', () => {
    render(<ShortcutHelp open onClose={() => {}} shortcuts={shortcuts} />);
    const fleet = screen.getByRole('heading', { name: 'Fleet' }).parentElement as HTMLElement;
    expect(within(fleet).getByText('Next browser')).toBeTruthy();
    expect(within(fleet).queryByText('Go to personas')).toBeNull();
  });
});
