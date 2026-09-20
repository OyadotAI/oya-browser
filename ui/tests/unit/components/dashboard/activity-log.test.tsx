/**
 * Unit tests for ActivityLog: what the browser has been doing.
 */
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ActivityLog from '@/components/dashboard/activity-log';
import type { Activity } from '@/components/dashboard/types';

afterEach(cleanup);

const ts = new Date().toISOString();
const items: Activity[] = [
  { ts, action: 'navigate', summary: 'to example.com', ok: true, ms: 12 },
  { ts, action: 'click', summary: '#4', ok: false, ms: 3, error: 'not found' },
];

describe('ActivityLog', () => {
  it('shows each command with its error beside the summary', () => {
    render(<ActivityLog items={items} optimistic={[]} now={Date.now()} />);
    expect(screen.getByText('to example.com')).toBeTruthy();
    expect(screen.getByText('#4 — not found')).toBeTruthy();
  });

  it('narrows to failures with "errors only"', async () => {
    render(<ActivityLog items={items} optimistic={[]} now={Date.now()} />);
    await userEvent.click(screen.getByRole('checkbox', { name: 'errors only' }));
    expect(screen.queryByText('to example.com')).toBeNull();
  });

  it('shows what the user just did before the server logs it', () => {
    render(<ActivityLog items={[]} optimistic={[{ ts, line: 'reload' }]} now={Date.now()} />);
    expect(screen.getByText('reload')).toBeTruthy();
    expect(screen.queryByText(/Nothing yet/)).toBeNull();
  });

  it('says so when nothing has happened yet', () => {
    render(<ActivityLog items={[]} optimistic={[]} now={Date.now()} />);
    expect(screen.getByText('Nothing yet. Click the live view, or send it somewhere.')).toBeTruthy();
  });
});
