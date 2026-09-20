/**
 * Unit tests for the example copy button: it copies, and says so, or says
 * how to copy by hand when the clipboard is refused.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import CopyExample from '@/components/copy-example';

afterEach(cleanup);

describe('CopyExample', () => {
  it('copies the code and announces it', async () => {
    const user = userEvent.setup();
    render(<CopyExample code="npm i" />);
    await user.click(screen.getByRole('button', { name: 'Copy example' }));
    expect(await navigator.clipboard.readText()).toBe('npm i');
    expect(screen.getByRole('status').textContent).toBe('Example copied.');
  });

  it('explains how to copy by hand when the clipboard refuses', async () => {
    const user = userEvent.setup();
    vi.spyOn(navigator.clipboard, 'writeText').mockRejectedValue(new Error('denied'));
    render(<CopyExample code="x" />);
    await user.click(screen.getByRole('button', { name: 'Copy example' }));
    expect(screen.getByRole('status').textContent).toContain('Could not copy.');
  });
});
