/**
 * Unit tests for connecting an app to Oya: the request waits through a sign-in,
 * the signed-in person sees which app asks and picks a project they can
 * operate, and allowing or turning it down posts that answer; a missing or
 * refused request says so.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import ConnectPage from '@/app/connect/page';
import { call } from '@/components/dashboard/projects/project-api';
import { afterSignIn } from '@/components/auth/use-auth-form';
import { createApiKey } from '@/lib/api';

const auth = { token: null as string | null, loading: false };
vi.mock('@/components/auth-provider', () => ({ useAuth: () => auth }));
vi.mock('next/navigation', () => ({ useRouter: () => ({ replace: vi.fn() }) }));
vi.mock('@/lib/api', () => ({ createApiKey: vi.fn(() => Promise.resolve({})) }));
vi.mock('@/components/dashboard/projects/project-api', () => ({
  call: vi.fn(),
  message: (e: unknown) => (e instanceof Error ? e.message : 'Something went wrong'),
}));

/** The projects the account holds: an owned one, a shared one, and one it can only view. */
const PROJECTS = [
  { id: 'prj_a', name: 'Acme', role: 'administrator' },
  { id: 'prj_b', name: 'Beta', role: 'operator' },
  { id: 'prj_v', name: 'Viewing', role: 'viewer' },
];

/** Answers the page's reads and records its answers. */
function serve(_token: string, path: string, init?: RequestInit) {
  if (path === '/auth/projects') return Promise.resolve(PROJECTS);
  if (init?.method === 'POST') return Promise.resolve({ redirect: 'https://claude.ai/cb?code=c' });
  return Promise.resolve({ client: 'Claude', redirectHost: 'claude.ai' });
}

/** Serves `projects` as the account's projects, everything else as usual. */
const withProjects = (projects: typeof PROJECTS) =>
  vi
    .mocked(call)
    .mockImplementation(((t: string, path: string, init?: RequestInit) =>
      path === '/auth/projects' ? Promise.resolve(projects) : serve(t, path, init)) as never);

/** Opens the link claude.ai sends the person to. */
const openLink = (request = 'req-1') =>
  window.history.replaceState(null, '', `/connect${request ? `?request=${request}` : ''}`);

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  sessionStorage.clear();
  localStorage.clear();
  auth.token = null;
});

describe('ConnectPage', () => {
  it('asks a signed-out person to sign in, then brings them back here', async () => {
    openLink();
    render(<ConnectPage />);
    expect(await screen.findByText('Connect an app to Oya')).toBeTruthy();
    expect(afterSignIn()).toBe('/connect');
    expect(call).not.toHaveBeenCalled();
  });

  it('names the app and offers only projects the person can operate', async () => {
    vi.mocked(call).mockImplementation(serve as never);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    expect(await screen.findByText('Allow Claude to use Oya Browser?')).toBeTruthy();
    const options = screen.getAllByRole('option').map((o) => o.textContent);
    expect(options).toEqual(['Acme', 'Beta']);
  });

  it('allows the request for the chosen project', async () => {
    vi.mocked(call).mockImplementation(serve as never);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    fireEvent.change(await screen.findByLabelText('Project'), { target: { value: 'prj_b' } });
    fireEvent.click(screen.getByRole('button', { name: 'Allow' }));
    await vi.waitFor(() =>
      expect(call).toHaveBeenCalledWith('jwt', '/auth/connect/req-1/allow', {
        method: 'POST',
        body: JSON.stringify({ project: 'prj_b' }),
      }),
    );
  });

  it('turns the request down on Cancel', async () => {
    vi.mocked(call).mockImplementation(serve as never);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    fireEvent.click(await screen.findByRole('button', { name: 'Cancel' }));
    await vi.waitFor(() =>
      expect(call).toHaveBeenCalledWith('jwt', '/auth/connect/req-1/deny', { method: 'POST', body: '{}' }),
    );
  });

  it('refuses to follow a redirect that is not http(s)', async () => {
    vi.mocked(call).mockImplementation(((_t: string, path: string, init?: RequestInit) =>
      init?.method === 'POST' ? Promise.resolve({ redirect: 'javascript:alert(1)' }) : serve(_t, path, init)) as never);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    fireEvent.click(await screen.findByRole('button', { name: 'Allow' }));
    expect((await screen.findByRole('alert')).textContent).toMatch(/unsafe address/);
  });

  it('lets a person with only viewer projects go back, so the app is not left waiting', async () => {
    withProjects([PROJECTS[2]]);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    expect(await screen.findByText('No project to connect')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Back to Claude' }));
    await vi.waitFor(() =>
      expect(call).toHaveBeenCalledWith('jwt', '/auth/connect/req-1/deny', { method: 'POST', body: '{}' }),
    );
  });

  it('gives an expired request its own screen, sending the person back to the app', async () => {
    vi.mocked(call).mockRejectedValue(Object.assign(new Error('expired'), { status: 410 }));
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    expect(await screen.findByText('This request has expired')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Allow' })).toBeNull();
  });

  it('offers to try again when the request could not be loaded', async () => {
    vi.mocked(call).mockRejectedValue(new Error('Could not load this request.'));
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    expect((await screen.findByRole('alert')).textContent).toBe(
      'Could not load this request. Try again, or start again from the app.',
    );
    expect(screen.getByRole('button', { name: 'Try again' })).toBeTruthy();
  });

  it('asks in one click when there is one project: no dropdown, the project named', async () => {
    withProjects([PROJECTS[1]]);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    expect(
      await screen.findByText('Claude can open and drive browsers in Beta. You return to claude.ai.'),
    ).toBeTruthy();
    expect(screen.queryByLabelText('Project')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Allow' }));
    await vi.waitFor(() =>
      expect(call).toHaveBeenCalledWith('jwt', '/auth/connect/req-1/allow', {
        method: 'POST',
        body: JSON.stringify({ project: 'prj_b' }),
      }),
    );
  });

  it('gives someone who just signed up their first project, so they can allow at once', async () => {
    const mine = [] as typeof PROJECTS;
    vi.mocked(createApiKey).mockImplementation((() => Promise.resolve(mine.push(PROJECTS[0]))) as never);
    withProjects(mine);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    expect(
      await screen.findByText('Claude can open and drive browsers in Acme. You return to claude.ai.'),
    ).toBeTruthy();
    expect(createApiKey).toHaveBeenCalledWith('jwt', 'My project');
  });

  it('starts on the project connected last time', async () => {
    localStorage.setItem('oya_connect_project', 'prj_b');
    vi.mocked(call).mockImplementation(serve as never);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    expect(((await screen.findByLabelText('Project')) as HTMLSelectElement).value).toBe('prj_b');
  });

  it('ignores a remembered project the person can no longer connect', async () => {
    localStorage.setItem('oya_connect_project', 'prj_gone');
    vi.mocked(call).mockImplementation(serve as never);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    expect(((await screen.findByLabelText('Project')) as HTMLSelectElement).value).toBe('prj_a');
  });

  it('remembers the project an app was connected to', async () => {
    vi.mocked(call).mockImplementation(serve as never);
    auth.token = 'jwt';
    openLink();
    render(<ConnectPage />);
    fireEvent.change(await screen.findByLabelText('Project'), { target: { value: 'prj_b' } });
    fireEvent.click(screen.getByRole('button', { name: 'Allow' }));
    await vi.waitFor(() => expect(localStorage.getItem('oya_connect_project')).toBe('prj_b'));
  });

  it('says so when the link carries no request', async () => {
    openLink('');
    render(<ConnectPage />);
    expect(await screen.findByText('No request in this link')).toBeTruthy();
  });
});
