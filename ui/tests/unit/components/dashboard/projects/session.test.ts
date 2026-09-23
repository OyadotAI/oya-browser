/**
 * Unit tests for the project session: opening a project, the newest switch
 * winning, falling back when a project cannot be opened, renewal, and the
 * request helpers behind them.
 */
import { createHash } from 'node:crypto';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { calls, stubFetch } from './fake-fetch';

vi.mock('@/lib/api', () => ({
  apiUrl: (p: string) => p,
  authHeaders: () => ({}),
  listApiKeys: vi.fn(),
  createApiKey: vi.fn(),
}));

import { createApiKey, listApiKeys } from '@/lib/api';
import { RENEW_INTERVAL_MS, PROJECT_CREDENTIAL, PROJECT_ID } from '@/components/dashboard/projects/constants';
import { call, projectIdFor } from '@/components/dashboard/projects/project-api';
import {
  loadProjects,
  openAny,
  openFirstProject,
  openProject,
  renew,
  startup,
  watchRenewal,
} from '@/components/dashboard/projects/session';
import type { Session } from '@/components/dashboard/projects/types';

/** A session whose effects are spies. */
function fakeSession(token: string | null = 't'): Session {
  return {
    token,
    setApiKey: vi.fn(),
    toast: vi.fn(),
    opening: { current: 0 },
    setCurrentId: vi.fn(),
    setListing: vi.fn(),
  };
}

/** The access route for a project. */
const access = (id: string) => `POST /auth/projects/${id}/access`;

describe('project session', () => {
  beforeEach(() => {
    sessionStorage.clear();
    vi.mocked(listApiKeys).mockResolvedValue([]);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it('a new account with no project gets one made and opened at sign-in', async () => {
    vi.mocked(createApiKey).mockResolvedValue({ key: 'k', project: 'p1' });
    stubFetch({ 'GET /auth/projects': [200, []], [access('p1')]: [200, { token: 'cred-p1' }] });
    const s = fakeSession();
    startup(s);
    await vi.waitFor(() => expect(s.setApiKey).toHaveBeenCalledWith('cred-p1', 'p1'));
    expect(createApiKey).toHaveBeenCalledWith('t', 'My project');
  });

  it('makes only one first project when sign-in runs twice at once', async () => {
    vi.mocked(createApiKey).mockClear().mockResolvedValue({ key: 'k', project: 'p1' });
    stubFetch({ 'GET /auth/projects': [200, []], [access('p1')]: [200, { token: 'c' }] });
    const s = fakeSession();
    await Promise.all([openFirstProject(s), openFirstProject(s)]);
    expect(createApiKey).toHaveBeenCalledTimes(1);
  });

  it('opening a project stores its credential and hands it to the console', async () => {
    stubFetch({ [access('a')]: [200, { token: 'cred-a' }] });
    const s = fakeSession();
    expect(await openProject(s, 'a')).toBe(true);
    expect(sessionStorage.getItem(PROJECT_CREDENTIAL)).toBe('cred-a');
    expect(sessionStorage.getItem(PROJECT_ID)).toBe('a');
    expect(s.setApiKey).toHaveBeenCalledWith('cred-a', 'a');
  });

  it('a switch overtaken by a newer one does not take effect', async () => {
    stubFetch({ [access('a')]: [200, { token: 'cred-a' }] });
    const s = fakeSession();
    const pending = openProject(s, 'a');
    s.opening.current++;
    expect(await pending).toBe(false);
    expect(s.setApiKey).not.toHaveBeenCalled();
  });

  it('a renewal never supersedes a switch', async () => {
    stubFetch({ [access('a')]: [200, { token: 'cred-a' }] });
    const s = fakeSession();
    await openProject(s, 'a', true);
    expect(s.opening.current).toBe(0);
  });

  it('opens nothing without an account token', async () => {
    const { spy } = stubFetch();
    expect(await openProject(fakeSession(null), 'a')).toBe(false);
    expect(await loadProjects(fakeSession(null))).toEqual([]);
    expect(spy).not.toHaveBeenCalled();
  });

  it('opens your own projects before shared ones', async () => {
    const { spy } = stubFetch({ [access('mine')]: [200, { token: 'c' }], [access('theirs')]: [200, { token: 'c' }] });
    const s = fakeSession();
    await openAny(s, [
      { id: 'theirs', name: 'T', role: 'member' },
      { id: 'mine', name: 'M', role: 'owner', owner: true },
    ]);
    expect(calls(spy)).toEqual([access('mine')]);
  });

  it('moves on to the next project when one cannot be opened', async () => {
    stubFetch({ [access('a')]: [500, { error: 'boom' }], [access('b')]: [200, { token: 'cred-b' }] });
    const s = fakeSession();
    await openAny(s, [
      { id: 'a', name: 'A', role: 'owner', owner: true },
      { id: 'b', name: 'B', role: 'owner', owner: true },
    ]);
    expect(s.setApiKey).toHaveBeenCalledWith('cred-b', 'b');
    expect(s.toast).not.toHaveBeenCalled();
  });

  it('forgets the credential and says why when no project opens', async () => {
    stubFetch({ [access('a')]: [500, { error: 'boom' }] });
    const s = fakeSession();
    sessionStorage.setItem(PROJECT_ID, 'a');
    await openAny(s, [{ id: 'a', name: 'A', role: 'owner', owner: true }]);
    expect(s.setApiKey).toHaveBeenCalledWith('', null);
    expect(sessionStorage.getItem(PROJECT_ID)).toBeNull();
    expect(s.toast).toHaveBeenCalledWith('No project could be opened: boom', 'error');
  });

  it('losing access on renewal moves the console to another project', async () => {
    stubFetch({
      [access('gone')]: [403, { error: 'no' }],
      'GET /auth/projects': [
        200,
        [
          { id: 'gone', name: 'G', role: 'owner', owner: true },
          { id: 'next', name: 'N', role: 'owner', owner: true },
        ],
      ],
      [access('next')]: [200, { token: 'cred-next' }],
    });
    const s = fakeSession();
    sessionStorage.setItem(PROJECT_ID, 'gone');
    await renew(s, 'gone');
    expect(s.toast).toHaveBeenCalledWith('You no longer have access to that project', 'info');
    expect(s.setApiKey).toHaveBeenLastCalledWith('cred-next', 'next');
  });

  it('a passing outage on renewal keeps the open project', async () => {
    stubFetch({ [access('a')]: [502, {}] });
    const s = fakeSession();
    sessionStorage.setItem(PROJECT_ID, 'a');
    await renew(s, 'a');
    expect(s.setApiKey).not.toHaveBeenCalled();
    expect(sessionStorage.getItem(PROJECT_ID)).toBe('a');
  });

  it('a stale renewal does not forget a project switched to meanwhile', async () => {
    stubFetch({ [access('old')]: [403, {}] });
    const s = fakeSession();
    sessionStorage.setItem(PROJECT_ID, 'new');
    await renew(s, 'old');
    expect(s.toast).not.toHaveBeenCalled();
  });

  it('renews on an interval and at once when the credential is refused', async () => {
    vi.useFakeTimers();
    const { spy } = stubFetch({ [access('a')]: [200, { token: 'c' }] });
    const stop = watchRenewal(fakeSession(), 'a');
    await vi.advanceTimersByTimeAsync(RENEW_INTERVAL_MS);
    expect(calls(spy)).toEqual([access('a')]);
    window.dispatchEvent(new CustomEvent('oya:credential-gone'));
    expect(spy).toHaveBeenCalledTimes(2);
    stop();
    await vi.advanceTimersByTimeAsync(RENEW_INTERVAL_MS);
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('a failed key listing still loads the projects', async () => {
    stubFetch({ 'GET /auth/projects': [200, [{ id: 'a', name: 'A', role: 'owner' }]] });
    vi.mocked(listApiKeys).mockRejectedValueOnce(new Error('down'));
    const s = fakeSession();
    expect(await loadProjects(s)).toHaveLength(1);
    expect(s.setListing).toHaveBeenCalledWith({ projects: [{ id: 'a', name: 'A', role: 'owner' }], keys: [] });
  });
});

describe('project requests', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('a failure carries the server reason, status and code', async () => {
    stubFetch({ 'GET /x': [409, { error: 'In use', code: 'busy' }] });
    await expect(call('t', '/x')).rejects.toMatchObject({ message: 'In use', status: 409, code: 'busy' });
  });

  it('a non-JSON error page falls back to the given message with its status', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({ ok: false, status: 502, json: async () => Promise.reject(new Error('html')) })),
    );
    await expect(call('t', '/x', {}, 'Could not load')).rejects.toMatchObject({
      message: 'Could not load',
      status: 502,
    });
  });

  it('derives the project id from the key the way the server does', async () => {
    const hex = createHash('sha256').update('secret-key').digest('hex');
    expect(await projectIdFor('secret-key')).toBe(`prj_${hex.slice(0, 24)}`);
  });
});
