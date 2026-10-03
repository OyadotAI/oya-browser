/**
 * Unit tests for the Meta pixel component: it loads and counts a page view on
 * a public page, and does neither on the console, where API keys show.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/react';

let pathname = '/';
vi.mock('next/navigation', () => ({ usePathname: () => pathname }));
const configureMetaPixel = vi.fn();
const loadMetaPixel = vi.fn();
const metaPageView = vi.fn();
vi.mock('@/lib/meta-pixel', () => ({
  configureMetaPixel: (id: string) => configureMetaPixel(id),
  loadMetaPixel: (id: string) => loadMetaPixel(id),
  metaPageView: (path: string) => metaPageView(path),
}));

const { MetaPixel } = await import('@/components/meta-pixel');

describe('MetaPixel', () => {
  beforeEach(() => (loadMetaPixel.mockClear(), metaPageView.mockClear()));

  it('loads the pixel and counts a page view on a public page', () => {
    pathname = '/docs';
    render(<MetaPixel id="123456789012345" />);
    expect(loadMetaPixel).toHaveBeenCalledWith('123456789012345');
    expect(metaPageView).toHaveBeenCalledWith('/docs');
  });

  it('neither loads nor counts on the console, the live view, the admin page or a claim link', () => {
    for (const path of ['/dashboard', '/live/oya-1', '/admin', '/claim', '/auth/callback']) {
      pathname = path;
      render(<MetaPixel id="123456789012345" />);
    }
    expect(loadMetaPixel).not.toHaveBeenCalled();
    expect(metaPageView).not.toHaveBeenCalled();
    expect(configureMetaPixel).toHaveBeenCalledWith('123456789012345');
  });
});
