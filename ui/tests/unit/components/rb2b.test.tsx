/**
 * Unit tests for the RB2B component: it loads on a public page and never on
 * the console, where API keys and live browsers show.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/react';

let pathname = '/';
vi.mock('next/navigation', () => ({ usePathname: () => pathname }));
const loadRb2b = vi.fn();
vi.mock('@/lib/rb2b', () => ({ loadRb2b: (id: string) => loadRb2b(id) }));

const { Rb2b } = await import('@/components/rb2b');

describe('Rb2b', () => {
  beforeEach(() => loadRb2b.mockClear());

  it('loads RB2B on a public page', () => {
    pathname = '/docs';
    render(<Rb2b id="1N5W0HJMYRO5" />);
    expect(loadRb2b).toHaveBeenCalledWith('1N5W0HJMYRO5');
  });

  it('never loads RB2B on the console or the live view', () => {
    for (const path of ['/dashboard', '/live/oya-1']) {
      pathname = path;
      render(<Rb2b id="1N5W0HJMYRO5" />);
    }
    expect(loadRb2b).not.toHaveBeenCalled();
  });
});
