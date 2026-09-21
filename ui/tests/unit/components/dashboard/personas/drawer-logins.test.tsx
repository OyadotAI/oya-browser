/**
 * Unit tests for "Move logins" in the profile drawer: export the persona's
 * cookies to a file, import them from one, or copy them from another profile.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<typeof import('@/lib/api-client')>()), api: vi.fn() }));

import { api } from '@/lib/api-client';
import { LoginsSection } from '@/components/dashboard/personas/drawer-logins';
import { ToastProvider } from '@/components/dashboard/toast';
import type { Persona } from '@/components/dashboard/types';

const mockApi = vi.mocked(api);
const COOKIE = { name: 'sid', value: 'v', domain: '.x.com' };
const persona = { id: 'p1', name: 'ops' } as Persona;
const others = { personas: [persona, { id: 'p2', name: 'research' }] };

/** Renders the section for persona p1. */
function setup() {
  const onChanged = vi.fn();
  render(
    <ToastProvider>
      <LoginsSection apiKey="k" persona={persona} onChanged={onChanged} />
    </ToastProvider>,
  );
  return { onChanged };
}

/** The calls made to paths starting with `prefix`. */
const callsTo = (prefix: string) => mockApi.mock.calls.filter(([path]) => path.startsWith(prefix));

describe('LoginsSection', () => {
  beforeEach(() => {
    mockApi.mockReset();
    mockApi.mockImplementation(async (path: string, opts) => {
      if (path === '/personas') return others;
      if (opts?.method === 'PUT') return { imported: 1, skipped: 0, total: 3 };
      return { persona: 'p1', cookies: [COOKIE] };
    });
    URL.createObjectURL = vi.fn(() => 'blob:jar');
    URL.revokeObjectURL = vi.fn();
  });
  afterEach(cleanup);

  it('exports the logins as a file named after the profile', async () => {
    const clicked: string[] = [];
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function (this: HTMLAnchorElement) {
      clicked.push(this.download);
    });
    setup();
    await userEvent.click(screen.getByRole('button', { name: 'Export' }));
    await waitFor(() => expect(clicked).toEqual(['ops-logins.json']));
    expect(callsTo('/pool/cookies')[0][0]).toBe('/pool/cookies?persona=p1&format=json');
    expect(await screen.findByText('1 cookie exported. Keep the file private: it holds live sessions.')).toBeTruthy();
  });

  it('imports a cookie file, a list or an export, and says what was merged', async () => {
    const { onChanged } = setup();
    const file = new File([JSON.stringify({ cookies: [COOKIE] })], 'jar.json', { type: 'application/json' });
    await userEvent.upload(screen.getByLabelText('Import a cookie file'), file);
    expect(await screen.findByText('1 imported, 0 skipped. 3 cookies in this profile now.')).toBeTruthy();
    expect(callsTo('/pool/cookies').at(-1)?.[1]).toMatchObject({ method: 'PUT', body: { cookies: [COOKIE] } });
    expect(onChanged).toHaveBeenCalled();
  });

  it('says so when the file holds no cookies', async () => {
    setup();
    const file = new File(['{"nope":1}'], 'bad.json', { type: 'application/json' });
    await userEvent.upload(screen.getByLabelText('Import a cookie file'), file);
    expect(await screen.findByText(/no list of cookies/i)).toBeTruthy();
    expect(callsTo('/pool/cookies').length).toBe(0);
  });

  it('offers nothing to copy from, without failing, when the profile list cannot be read', async () => {
    mockApi.mockImplementation(async () => ({}));
    setup();
    expect(await screen.findByLabelText('Copy logins from another profile')).toBeTruthy();
    expect(screen.getAllByRole('option')).toHaveLength(1);
  });

  it('copies the logins of another profile, never offering this one', async () => {
    setup();
    const select = await screen.findByLabelText('Copy logins from another profile');
    await waitFor(() => expect(screen.getByRole('option', { name: 'research' })).toBeTruthy());
    expect(screen.queryByRole('option', { name: 'ops' })).toBeNull();
    await userEvent.selectOptions(select, 'p2');
    await userEvent.click(screen.getByRole('button', { name: 'Copy' }));
    expect(await screen.findByText('1 imported, 0 skipped. 3 cookies in this profile now.')).toBeTruthy();
    expect(callsTo('/pool/cookies').map(([path, o]) => [o?.method ?? 'GET', path])).toEqual([
      ['GET', '/pool/cookies?persona=p2&format=json'],
      ['PUT', '/pool/cookies?persona=p1'],
    ]);
  });
});
