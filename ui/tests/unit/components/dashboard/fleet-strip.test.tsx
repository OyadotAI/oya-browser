/**
 * Unit tests for FleetStrip: every number is a filter.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import FleetStrip from '@/components/dashboard/fleet-strip';
import type { Fleet } from '@/components/dashboard/types';
import { noFilter } from './fleet/fixtures';

afterEach(cleanup);

/** A fleet summary with the given persona counts. */
const fleet = (byPersona: Record<string, number> = {}): Fleet => ({
  at: '',
  uptimeSeconds: 0,
  browsers: {
    total: 3,
    byClient: {},
    byProvider: { steel: 2, anchor: 1 },
    byHealth: { ok: 2, stale: 0, errors: 1, dead: 0 },
    byPersona,
    commands: 0,
    errors: 0,
    pending: 0,
  },
});

describe('FleetStrip', () => {
  it('filters by a health chip, and clears it when the active chip is clicked', async () => {
    const onFilter = vi.fn();
    const { rerender } = render(<FleetStrip fleet={fleet()} rate={null} filter={noFilter} onFilter={onFilter} />);
    await userEvent.click(screen.getByTitle('1 errors'));
    expect(onFilter).toHaveBeenLastCalledWith({ health: 'errors' });
    rerender(<FleetStrip fleet={fleet()} rate={null} filter={{ ...noFilter, health: 'errors' }} onFilter={onFilter} />);
    await userEvent.click(screen.getByTitle('1 errors'));
    expect(onFilter).toHaveBeenLastCalledWith({ health: null });
  });

  it('lists providers busiest first', () => {
    render(<FleetStrip fleet={fleet()} rate={null} filter={noFilter} onFilter={vi.fn()} />);
    const group = screen.getByRole('group', { name: 'Filter by provider' });
    expect(group.textContent).toBe('2Steel1Anchor');
  });

  it('shows the top five personas and sums the rest', () => {
    const personas = Object.fromEntries(Array.from({ length: 7 }, (_, i) => [`p${i}`, 7 - i]));
    render(<FleetStrip fleet={fleet(personas)} rate={null} filter={noFilter} onFilter={vi.fn()} />);
    expect(screen.getByText('+2 more')).toBeTruthy();
  });

  it('turns the error rate red at the alert level', () => {
    render(
      <FleetStrip fleet={fleet()} rate={{ commandsPerMin: 10, errorPct: 5 }} filter={noFilter} onFilter={vi.fn()} />,
    );
    expect(screen.getByText('5.0%').className).toBe('text-red');
  });

  it('shows dashes before the first poll', () => {
    render(<FleetStrip fleet={null} rate={null} filter={noFilter} onFilter={vi.fn()} />);
    expect(screen.getAllByText('—')).toHaveLength(3);
  });
});
