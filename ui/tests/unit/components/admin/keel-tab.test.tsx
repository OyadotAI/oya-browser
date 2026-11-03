/** Verifies independent loading, missing measurements, safe refresh and credential changes in Keel's real view. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { KeelTab } from '@/components/admin/keel-tab';
import { adminKeel } from '@/lib/api';
import { KEEL_SNAPSHOT } from './keel-fixture';

const auth = { token: 'first-token' as string | null };
vi.mock('@/components/auth-provider', () => ({ useAuth: () => auth }));
vi.mock('@/lib/api', () => ({ adminKeel: vi.fn() }));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  auth.token = 'first-token';
});

describe('Keel tab', () => {
  it('keeps a previous snapshot and offers retry when refresh fails', async () => {
    vi.mocked(adminKeel).mockResolvedValueOnce(KEEL_SNAPSHOT).mockRejectedValueOnce(new Error('Offline'));
    render(<KeelTab />);
    expect(await screen.findByText('117 installations seen in total.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Refresh Keel usage' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Couldn’t refresh. Showing data updated');
    expect(screen.getByText('117 installations seen in total.')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Retry Keel usage' })).toBeTruthy();
  });

  it('shows missing hours as unavailable and describes reported IDs without validity claims', async () => {
    vi.mocked(adminKeel).mockResolvedValue({
      ...KEEL_SNAPSHOT,
      usage: { ...KEEL_SNAPSHOT.usage, runtime_ms: null, projects: null, runtime_reports: 0, project_reports: 0 },
    });
    render(<KeelTab />);
    expect(await screen.findByText('Reported IDs, not issued keys.')).toBeTruthy();
    expect(screen.getAllByLabelText('Not reported').length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText('None reported')).toBeTruthy();
    expect(screen.queryByText('Unlicensed')).toBeNull();
    expect(screen.getByText(/Existing reports do not include these measurements/)).toBeTruthy();
  });

  it('offers retry on an initial failure without inventing headline counts', async () => {
    vi.mocked(adminKeel).mockRejectedValue(new Error('Network unavailable'));
    render(<KeelTab />);
    expect((await screen.findByRole('alert')).textContent).toContain('Keel usage could not be loaded.');
    expect(screen.queryByText('117 installations seen in total.')).toBeNull();
    expect(screen.getByRole('button', { name: 'Retry Keel usage' })).toBeTruthy();
  });

  it('hides old credentials immediately and ignores their delayed response', async () => {
    let oldResponse: (value: unknown) => void = () => {};
    vi.mocked(adminKeel)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            oldResponse = resolve;
          }),
      )
      .mockResolvedValueOnce({ ...KEEL_SNAPSHOT, installs: { ...KEEL_SNAPSHOT.installs, total: 2 } });
    const view = render(<KeelTab />);
    auth.token = 'second-token';
    view.rerender(<KeelTab />);
    expect(await screen.findByText('2 installations seen in total.')).toBeTruthy();
    oldResponse(KEEL_SNAPSHOT);
    await waitFor(() => expect(screen.queryByText('117 installations seen in total.')).toBeNull());
    expect(screen.getByText('2 installations seen in total.')).toBeTruthy();
  });
});
