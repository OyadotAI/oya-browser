/**
 * Unit tests for the project switcher as a user meets it: the list, switching,
 * creating, joining, restoring, deleting, and the keyboard.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { cleanup, render, screen, waitFor, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { calls, stubFetch, type Answer } from './projects/fake-fetch';

/** The toast spy; stable across renders, as the real one is. */
const toast = vi.fn();
/** The account token useAuth hands out. */
let token: string | null = 't';

vi.mock('@/lib/api', () => ({
  apiUrl: (p: string) => p,
  authHeaders: () => ({}),
  listApiKeys: vi.fn(async () => ({ keys: [{ id: 'k', prefix: 'oya_ab', project: 'mine' }] })),
  createApiKey: vi.fn(),
  importApiKey: vi.fn(),
}));
vi.mock('@/components/auth-provider', () => ({ useAuth: () => ({ token }) }));
vi.mock('@/components/dashboard/toast', () => ({ useToast: () => toast }));

import { createApiKey, importApiKey } from '@/lib/api';
import ProjectSwitcher from '@/components/dashboard/project-switcher';

/** The account's projects. */
const PROJECTS = [
  { id: 'mine', name: 'Checkout', role: 'owner', owner: true },
  { id: 'shared', name: 'Support', role: 'viewer' },
];

/** Routes every test starts with: the list, and access to both projects. */
function baseRoutes(): Record<string, Answer> {
  return {
    'GET /auth/projects': [200, PROJECTS],
    'POST /auth/projects/mine/access': [200, { token: 'cred-mine' }],
    'POST /auth/projects/shared/access': [200, { token: 'cred-shared' }],
  };
}

/** Renders the switcher, waits for the first project to open, and opens the popover. */
async function setup(extra: Record<string, Answer> = {}) {
  const fetch = stubFetch({ ...baseRoutes(), ...extra });
  const setApiKey = vi.fn();
  render(<ProjectSwitcher apiKey="" setApiKey={setApiKey} />);
  await waitFor(() => expect(setApiKey).toHaveBeenCalledWith('cred-mine', 'mine'));
  await userEvent.click(screen.getByRole('button', { name: 'Switch project' }));
  return { ...fetch, setApiKey };
}

describe('ProjectSwitcher', () => {
  beforeEach(() => {
    token = 't';
    sessionStorage.clear();
    toast.mockReset();
  });
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  it('renders nothing for key-only sign-in', () => {
    token = null;
    const { container } = render(<ProjectSwitcher apiKey="k" setApiKey={vi.fn()} />);
    expect(container.innerHTML).toBe('');
  });

  it('lists your projects apart from shared ones, and shows the open one on the button', async () => {
    await setup();
    expect(screen.getByText('Your projects')).toBeTruthy();
    expect(screen.getByText('Shared with you')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Switch project' }).textContent).toContain('Checkout');
    expect(screen.getByLabelText('Selected')).toBeTruthy();
  });

  it('search narrows the list and says when nothing matches', async () => {
    await setup();
    await userEvent.type(screen.getByLabelText('Search projects'), 'zzz');
    expect(screen.getByText('No matching projects')).toBeTruthy();
  });

  it('switching opens the project and says so', async () => {
    const { setApiKey } = await setup();
    await userEvent.click(screen.getByRole('button', { name: /Support/ }));
    await waitFor(() => expect(setApiKey).toHaveBeenLastCalledWith('cred-shared', 'shared'));
    expect(toast).toHaveBeenCalledWith('Switched to Support', 'info');
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('a failed switch shows the server reason in the popover', async () => {
    await setup({ 'POST /auth/projects/shared/access': [403, { error: 'Access removed' }] });
    await userEvent.click(screen.getByRole('button', { name: /Support/ }));
    expect((await screen.findByRole('alert')).textContent).toContain('Access removed');
  });

  it('creating a project shows its key before anything else', async () => {
    vi.mocked(createApiKey).mockResolvedValueOnce({ key: 'oya_new', project: 'mine' });
    await setup();
    await userEvent.click(screen.getByRole('button', { name: /Create project/ }));
    await userEvent.type(screen.getByPlaceholderText('e.g. Checkout agents'), 'New one');
    await userEvent.click(screen.getByRole('button', { name: 'Create project' }));
    expect(((await screen.findByDisplayValue('oya_new')) as HTMLInputElement).readOnly).toBe(true);
    expect(createApiKey).toHaveBeenCalledWith('t', 'New one');
    expect(toast).toHaveBeenCalledWith('Project created. Copy your API key below.', 'success');
  });

  it('joining with an invitation code opens the project', async () => {
    const { spy } = await setup({ 'POST /auth/projects/join': [200, { project: 'shared' }] });
    await userEvent.click(screen.getByRole('button', { name: /Join with invite/ }));
    await userEvent.type(screen.getByPlaceholderText('Paste invitation code'), ' INV ');
    await userEvent.click(screen.getByRole('button', { name: 'Join project' }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith('Joined Support', 'success'));
    const join = spy.mock.calls.find(([url]) => url === '/auth/projects/join');
    expect(join?.[1]?.body).toBe(JSON.stringify({ code: 'INV' }));
  });

  it('restoring refuses a key that belongs to another project', async () => {
    await setup({ 'POST /auth/projects/mine/key': [500, { error: 'sealed', code: 'project_key_unavailable' }] });
    await userEvent.click(screen.getByRole('button', { name: 'Options for Checkout' }));
    await userEvent.click(screen.getByRole('button', { name: 'Copy API key for Checkout' }));
    await userEvent.click(await screen.findByRole('button', { name: /Restore with API key/ }));
    await userEvent.type(screen.getByPlaceholderText('Paste your API key'), 'wrong-key');
    await userEvent.click(screen.getByRole('button', { name: 'Restore access' }));
    expect((await screen.findByRole('alert')).textContent).toContain('That key doesn’t belong to Checkout');
    expect(importApiKey).not.toHaveBeenCalled();
  });

  it('a shared project with an unreadable key points at its owner', async () => {
    await setup({ 'POST /auth/projects/shared/access': [500, { code: 'project_key_unavailable' }] });
    await userEvent.click(screen.getByRole('button', { name: /Support/ }));
    expect((await screen.findByRole('alert')).textContent).toContain('Ask the project owner');
  });

  it('deleting the open project stops its browsers and opens another', async () => {
    const { spy, setApiKey } = await setup({ 'DELETE /auth/projects/mine': [200, {}] });
    await userEvent.click(screen.getByRole('button', { name: 'Options for Checkout' }));
    await userEvent.click(screen.getByRole('button', { name: 'Delete Checkout' }));
    await userEvent.click(screen.getByRole('button', { name: 'Delete project' }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith('Checkout deleted', 'success'));
    expect(calls(spy)).toContain('DELETE /auth/projects/mine');
    expect(setApiKey).toHaveBeenCalledWith('', null);
  });

  it('Escape closes the popover and returns focus to the button', async () => {
    await setup();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Switch project' }));
  });

  it('arrow keys move between projects and wrap around', async () => {
    await setup();
    const options = screen
      .getAllByRole('button', { name: /Checkout|Support/ })
      .filter((b) => b.hasAttribute('data-project-option'));
    options[0].focus();
    await userEvent.keyboard('{ArrowUp}');
    expect(document.activeElement).toBe(options[1]);
  });

  it('a click outside closes the popover', async () => {
    await setup();
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
