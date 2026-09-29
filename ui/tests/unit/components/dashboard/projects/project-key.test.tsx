/**
 * Unit tests for the show-and-copy API key button in Settings and onboarding:
 * it fetches the open project's key only when asked, copies it, and says so
 * plainly when the person is not the project's owner.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/components/auth-provider', () => ({ useAuth: () => ({ token: 'account-token' }) }));

import { ProjectKey } from '@/components/dashboard/project-switcher';

/** Answers every fetch with `status` and `body`, and records the calls. */
function answer(status: number, body: object) {
  const fetchMock = vi.fn<typeof fetch>(async () => new Response(JSON.stringify(body), { status }));
  vi.stubGlobal('fetch', fetchMock);
  return fetchMock;
}

describe('ProjectKey', () => {
  afterEach(() => (cleanup(), vi.unstubAllGlobals()));

  it('fetches nothing until asked, then shows and copies the key', async () => {
    const fetchMock = answer(200, { key: 'oya-key-1' });
    const writeText = vi.fn(async () => {});
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    render(<ProjectKey projectId="prj_1" />);
    expect(fetchMock).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: /Show and copy API key/ }));
    expect((await screen.findByLabelText('API key')).getAttribute('value')).toBe('oya-key-1');
    expect(String(fetchMock.mock.calls[0][0])).toContain('/auth/projects/prj_1/key');
    expect(writeText).toHaveBeenCalledWith('oya-key-1');
    expect(screen.getByRole('button', { name: /Copied to clipboard/ })).toBeTruthy();
  });

  it('tells a member who is not the owner why there is no key', async () => {
    answer(403, { error: 'Forbidden' });
    render(<ProjectKey projectId="prj_1" />);
    await userEvent.click(screen.getByRole('button', { name: /Show and copy API key/ }));
    expect((await screen.findByRole('alert')).textContent).toBe('Only the project owner can see its API key.');
  });

  it('is disabled without an open project', () => {
    answer(200, {});
    render(<ProjectKey projectId={null} />);
    expect(screen.getByRole('button', { name: /Show and copy API key/ }).hasAttribute('disabled')).toBe(true);
  });
});
