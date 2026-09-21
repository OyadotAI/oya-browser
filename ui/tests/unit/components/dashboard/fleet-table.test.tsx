/**
 * Unit tests for FleetTable: what the user sees and what their clicks call.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { createRef, type ComponentProps } from 'react';
import FleetTable from '@/components/dashboard/fleet-table';
import { desktopSignInUrl } from '@/components/dashboard/config';
import { api } from '@/lib/api-client';
import { noFilter, row } from './fleet/fixtures';

const toast = vi.fn();
vi.mock('@/components/dashboard/toast', () => ({ useToast: () => toast }));
vi.mock('@/components/dashboard/config', () => ({ desktopSignInUrl: vi.fn() }));
vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

afterEach(() => {
  cleanup();
  toast.mockReset();
});

/** Renders the table with spies for every callback. */
function setup(over: Partial<ComponentProps<typeof FleetTable>> = {}) {
  const props: ComponentProps<typeof FleetTable> = {
    rows: [row({ id: 'b-1', name: 'alpha' }), row({ id: 'b-2', name: 'beta', health: 'errors' })],
    selectedId: null,
    onSelect: vi.fn(),
    checked: new Set(),
    onChecked: vi.fn(),
    filter: noFilter,
    onFilter: vi.fn(),
    onStop: vi.fn(),
    onStart: vi.fn(),
    onConnect: vi.fn(),
    onScreenshot: vi.fn(),
    onCode: vi.fn(),
    apiKey: 'k',
    filterRef: createRef(),
    now: Date.now(),
    ...over,
  };
  render(<FleetTable {...props} />);
  return props;
}

describe('FleetTable', () => {
  it('lists browsers needing attention first', () => {
    setup();
    const names = screen
      .getAllByRole('row')
      .slice(1)
      .map((r) => r.getAttribute('data-id'));
    expect(names).toEqual(['b-2', 'b-1']);
  });

  it('scrolls the selected row into view', () => {
    const scroll = vi.fn();
    Element.prototype.scrollIntoView = scroll;
    setup({ selectedId: 'b-2' });
    expect(scroll).toHaveBeenCalledWith({ block: 'nearest' });
  });

  it('selects a row on click, and clears the selection on a second click', async () => {
    Element.prototype.scrollIntoView = vi.fn();
    const p = setup({ selectedId: 'b-1' });
    await userEvent.click(screen.getByTitle('alpha'));
    expect(p.onSelect).toHaveBeenCalledWith(null);
    await userEvent.click(screen.getByTitle('beta'));
    expect(p.onSelect).toHaveBeenCalledWith('b-2');
  });

  it('stops every browser from "Stop all", and the checked ones once any are checked', async () => {
    const p = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Stop all' }));
    expect(p.onStop).toHaveBeenCalledWith(['b-1', 'b-2']);
    cleanup();
    const q = setup({ checked: new Set(['b-2']) });
    await userEvent.click(screen.getByRole('button', { name: /Stop 1/ }));
    expect(q.onStop).toHaveBeenCalledWith(['b-2']);
  });

  it('invites the first browser when none are running', async () => {
    const p = setup({ rows: [] });
    await userEvent.click(screen.getByRole('button', { name: 'Start a browser' }));
    expect(p.onStart).toHaveBeenCalled();
  });

  it('offers to clear filters that hide every row', async () => {
    const p = setup({ filter: { ...noFilter, text: 'nothing-matches' } });
    await userEvent.click(screen.getByRole('button', { name: 'Clear filters' }));
    expect(p.onFilter).toHaveBeenCalledWith(noFilter);
  });

  it('opens the row menu on right-click, with every action for that browser', async () => {
    setup();
    await userEvent.pointer({ keys: '[MouseRight]', target: screen.getByTitle('alpha') });
    expect(screen.getByRole('menu', { name: 'Actions for alpha' })).toBeTruthy();
    expect(screen.getByRole('menuitem', { name: /Copy CDP attach URL, not a CDP browser/ })).toBeTruthy();
  });

  it('toasts why desktop pairing failed', async () => {
    vi.mocked(desktopSignInUrl).mockRejectedValueOnce(new Error('no pairing'));
    setup();
    await userEvent.click(screen.getByRole('button', { name: /Connect desktop browser/ }));
    expect(toast).toHaveBeenCalledWith('no pairing', 'error');
  });

  it('closes the blank stream tab when the token cannot be minted', async () => {
    const tab = { close: vi.fn(), location: {} };
    vi.spyOn(window, 'open').mockReturnValue(tab as unknown as Window);
    vi.mocked(api).mockRejectedValueOnce(new Error('denied'));
    setup();
    await userEvent.pointer({ keys: '[MouseRight]', target: screen.getByTitle('alpha') });
    await userEvent.click(screen.getByRole('menuitem', { name: /Open live stream/ }));
    await vi.waitFor(() => expect(tab.close).toHaveBeenCalled());
    expect(toast).toHaveBeenCalledWith('Could not open the live stream: denied', 'error');
  });

  it('confirms a copy, and says so when the clipboard refuses', async () => {
    const writeText = vi.fn().mockResolvedValueOnce(undefined).mockRejectedValueOnce(new Error('blocked'));
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    setup();
    for (const outcome of [
      ['Browser id copied', 'success'],
      ['Could not copy: blocked', 'error'],
    ]) {
      await userEvent.pointer({ keys: '[MouseRight]', target: screen.getByTitle('alpha') });
      await userEvent.click(screen.getByRole('menuitem', { name: 'Copy browser id' }));
      await vi.waitFor(() => expect(toast).toHaveBeenCalledWith(...outcome));
    }
    expect(writeText).toHaveBeenCalledWith('b-1');
  });
});
