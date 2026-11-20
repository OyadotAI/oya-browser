/**
 * Unit tests for ConsoleBody workspace-tab keying across project switches,
 * manual API-key switches, and hourly credential renewals.
 */
import { useState } from 'react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { ConsoleBody } from '@/app/dashboard/_console/console-body';
import type { Console } from '@/app/dashboard/_console/use-console';

/** Minimal stateful stand-in for PersonasTab so remounting clears local modal state. */
function PersonasStub() {
  const [open, setOpen] = useState(false);
  return (
    <div>
      <button type="button" onClick={() => setOpen(true)}>
        New profile
      </button>
      {open && <div>New profile dialog</div>}
    </div>
  );
}

vi.mock('@/components/dashboard/personas-tab', () => ({ default: PersonasStub }));

afterEach(() => {
  cleanup();
});

/** Builds minimal Console state positioned on the personas tab. */
function stateFor(project: string | null, apiKey: string): Console {
  return {
    apiKey,
    project,
    config: null,
    refreshError: null,
    view: {
      tab: 'personas',
      selected: null,
      openPersona: null,
      filter: '',
      connectId: null,
      shot: null,
      showStart: false,
      showSetup: false,
      stopId: null,
      bannerDismissed: true,
    },
    patch: vi.fn(),
    browsers: [],
    personas: [],
    personasStatus: 'ready',
    fetchPersonas: vi.fn(),
    showBrowsersFor: vi.fn(),
    now: 0,
    fleet: null,
    rate: null,
    onFilter: vi.fn(),
    onClearFilter: vi.fn(),
    requestStop: vi.fn(),
    browserError: null,
    browserStatus: 'ready',
    fetchBrowsers: vi.fn(),
    dismissNotice: vi.fn(),
    notices: [],
    selectedBrowser: null,
    filterRef: { current: null },
    urlRef: { current: null },
  } as unknown as Console;
}

describe('ConsoleBody', () => {
  it('remounts PersonasTab when switching between manual API keys without a project', () => {
    const { rerender } = render(<ConsoleBody c={stateFor(null, 'k-1')} />);
    fireEvent.click(screen.getByRole('button', { name: 'New profile' }));
    expect(screen.getByText('New profile dialog')).toBeTruthy();

    rerender(<ConsoleBody c={stateFor(null, 'k-2')} />);
    expect(screen.queryByText('New profile dialog')).toBeNull();
  });

  it('keeps PersonasTab mounted on credential renewal within the same project, but remounts on project switch', () => {
    const { rerender } = render(<ConsoleBody c={stateFor('prj-1', 'cred-1')} />);
    fireEvent.click(screen.getByRole('button', { name: 'New profile' }));
    expect(screen.getByText('New profile dialog')).toBeTruthy();

    rerender(<ConsoleBody c={stateFor('prj-1', 'cred-2')} />);
    expect(screen.getByText('New profile dialog')).toBeTruthy();

    rerender(<ConsoleBody c={stateFor('prj-2', 'cred-3')} />);
    expect(screen.queryByText('New profile dialog')).toBeNull();
  });
});
