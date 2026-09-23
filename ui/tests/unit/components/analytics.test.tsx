/**
 * Unit tests for the analytics component: tagged clicks count on public pages
 * once the library has loaded, and the console is never listened on.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, waitFor } from '@testing-library/react';

let pathname = '/';
vi.mock('next/navigation', () => ({ usePathname: () => pathname }));
vi.mock('@/components/auth-provider', () => ({ useAuth: () => ({ user: null }) }));
const trackedClick = vi.fn();
vi.mock('@/lib/analytics', () => ({
  init: async () => {},
  pageview: () => {},
  identify: () => {},
  reset: () => {},
  trackedClick: (target: EventTarget | null) => trackedClick(target),
}));

const { Analytics } = await import('@/components/analytics');

/** Renders the component on `path`, waits for the library, and clicks the page. */
async function clickOn(path: string) {
  pathname = path;
  const view = render(<Analytics posthogKey="phc_test" host="https://ph.example.test" />);
  await waitFor(() => Promise.resolve());
  await new Promise((r) => setTimeout(r, 0));
  document.body.click();
  view.unmount();
}

describe('Analytics', () => {
  beforeEach(() => trackedClick.mockClear());

  it('counts clicks on a public page', async () => {
    await clickOn('/');
    expect(trackedClick).toHaveBeenCalledTimes(1);
  });

  it('never listens for clicks on the console', async () => {
    await clickOn('/dashboard');
    expect(trackedClick).not.toHaveBeenCalled();
  });
});
