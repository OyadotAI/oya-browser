/**
 * Unit tests for the context menu: arrows skip disabled items and wrap,
 * Enter picks, Escape and outside clicks close, and it stays on screen.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ContextMenu, { type MenuItem } from '@/components/ui/context-menu';

afterEach(cleanup);

/** Three items, the middle one disabled. */
const items = (): MenuItem[] => [
  { label: 'Open', onSelect: vi.fn() },
  { label: 'Pinned', disabled: true, onSelect: vi.fn() },
  { label: 'Stop', danger: true, separator: true, shortcut: 'x', onSelect: vi.fn() },
];

describe('ContextMenu', () => {
  it('renders nothing while closed', () => {
    render(<ContextMenu at={null} items={items()} onClose={() => {}} label="Row" />);
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('arrows skip disabled items and wrap; Enter picks and closes', async () => {
    const list = items();
    const onClose = vi.fn();
    render(<ContextMenu at={{ x: 10, y: 10 }} items={list} onClose={onClose} label="Row" />);
    await userEvent.keyboard('{ArrowDown}{Enter}');
    expect(list[2].onSelect).toHaveBeenCalledOnce();
    // From Stop: down wraps to Open, down skips Pinned to Stop, up back to Open.
    await userEvent.keyboard('{ArrowDown}{ArrowDown}{ArrowUp}{Enter}');
    expect(list[0].onSelect).toHaveBeenCalledOnce();
    expect(list[1].onSelect).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('never picks a disabled item by click', async () => {
    const list = items();
    render(<ContextMenu at={{ x: 0, y: 0 }} items={list} onClose={() => {}} label="Row" />);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Pinned' }));
    expect(list[1].onSelect).not.toHaveBeenCalled();
  });

  it('closes on Escape and on a click outside, not on a click inside', async () => {
    const onClose = vi.fn();
    render(<ContextMenu at={{ x: 0, y: 0 }} items={items()} onClose={onClose} label="Row" />);
    fireEvent.mouseDown(screen.getByRole('menu'));
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.mouseDown(document.body);
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('keeps itself on screen near the viewport edge', () => {
    render(<ContextMenu at={{ x: 5000, y: -20 }} items={items()} onClose={() => {}} label="Row" />);
    const menu = screen.getByRole('menu', { name: 'Row' });
    expect(menu.style.left).toBe(`${window.innerWidth - 8}px`);
    expect(menu.style.top).toBe('8px');
  });
});
