/**
 * Unit tests for the Groups dialog (groups-dialog.tsx): loading, a failed load
 * with Try again, the empty state, making a group of non-default profiles, the
 * server's refusal kept in the editor, member and running counts, and deleting.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

import { api, ApiError } from '@/lib/api-client';
import GroupsDialog from '@/components/dashboard/groups-dialog';
import { ToastProvider } from '@/components/dashboard/toast';
import type { Persona } from '@/components/dashboard/types';

const apiMock = vi.mocked(api);

/** A profile with `over` laid on. */
const persona = (over: Partial<Persona>): Persona =>
  ({ id: 'p', name: 'p', isDefault: false, activeBrowsers: 0, maxConcurrent: 1, ...over }) as Persona;
const PERSONAS = [
  persona({ id: 'p-def', name: 'Default', isDefault: true, maxConcurrent: null }),
  persona({ id: 'p-a', name: 'Alpha', activeBrowsers: 1 }),
  persona({ id: 'p-b', name: 'Beta', maxConcurrent: 2 }),
];
const POOL = { name: 'pool', personaIds: ['p-a', 'p-b'], createdAt: 'c', updatedAt: 'u' };

/** Answers GET /persona-groups with `groups`, and `other` for anything else. */
function serve(
  groups: unknown[],
  other: (path: string, opts: { method?: string; body?: unknown }) => unknown = () => ({}),
) {
  apiMock.mockImplementation(async (path, opts) =>
    path === '/persona-groups' && !opts?.method ? { groups } : other(path, opts),
  );
}

/** Renders the dialog open and waits for its load. */
async function setup() {
  render(
    <ToastProvider>
      <GroupsDialog open onClose={() => {}} apiKey="k" personas={PERSONAS} browsers={[]} />
    </ToastProvider>,
  );
  await act(async () => {});
}

describe('GroupsDialog', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });
  afterEach(() => cleanup());

  it('says it could not load the groups, and loads them on Try again', async () => {
    apiMock.mockRejectedValueOnce(new Error('down'));
    await setup();
    expect(screen.getByText('Could not load your groups')).toBeTruthy();
    serve([POOL]);
    await userEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(screen.getByRole('table', { name: 'Groups' })).toBeTruthy();
  });

  it('shows what a group is for when there are none', async () => {
    serve([]);
    await setup();
    expect(screen.getByText('No groups yet')).toBeTruthy();
    expect(screen.getByText(/personaGroup: 'my-group'/)).toBeTruthy();
  });

  it('lists a group with its profiles and how many run across their caps', async () => {
    serve([POOL]);
    await setup();
    const row = screen.getByRole('row', { name: /pool/ });
    expect(within(row).getByText('Alpha, Beta')).toBeTruthy();
    expect(within(row).getByText('1/3')).toBeTruthy();
  });

  it('makes a group of the ticked profiles, offering every one but the default', async () => {
    serve([], (path, opts) => (path === '/persona-groups' && opts.method === 'POST' ? POOL : {}));
    await setup();
    await userEvent.click(screen.getByRole('button', { name: /New group/ }));
    expect(screen.queryByRole('checkbox', { name: /Default/ })).toBeNull();
    const save = screen.getByRole('button', { name: 'Create group' }) as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    await userEvent.type(screen.getByLabelText('Name'), 'pool');
    await userEvent.click(screen.getByRole('checkbox', { name: /Alpha/ }));
    await userEvent.click(save);
    expect(apiMock).toHaveBeenCalledWith('/persona-groups', {
      key: 'k',
      method: 'POST',
      body: { name: 'pool', personaIds: ['p-a'] },
    });
  });

  it('keeps the server’s refusal in the editor', async () => {
    serve([], () => {
      throw new ApiError('A persona group named pool already exists', 409, null);
    });
    await setup();
    await userEvent.click(screen.getByRole('button', { name: /New group/ }));
    await userEvent.type(screen.getByLabelText('Name'), 'pool');
    await userEvent.click(screen.getByRole('checkbox', { name: /Beta/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Create group' }));
    expect(screen.getByRole('alert').textContent).toBe('A persona group named pool already exists');
  });

  it('changes a group’s members without renaming it', async () => {
    serve([POOL]);
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Edit pool' }));
    expect(screen.queryByLabelText('Name')).toBeNull();
    await userEvent.click(screen.getByRole('checkbox', { name: /Beta/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(apiMock).toHaveBeenCalledWith('/persona-groups/pool', {
      key: 'k',
      method: 'PUT',
      body: { personaIds: ['p-a'] },
    });
  });

  it('asks before deleting, then deletes', async () => {
    serve([POOL]);
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Delete pool' }));
    expect(screen.getByText('Delete group pool?')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Delete group' }));
    expect(apiMock).toHaveBeenCalledWith('/persona-groups/pool', { key: 'k', method: 'DELETE' });
  });
});
