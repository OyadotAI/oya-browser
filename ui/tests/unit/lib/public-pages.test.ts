/**
 * Unit tests for which pages count as public: visitor tracking runs on the
 * landing page, docs and sign-in, never on the console or the live view.
 */
import { describe, it, expect } from 'vitest';
import { isPublicPage } from '@/lib/public-pages';

describe('isPublicPage', () => {
  it('counts the landing page, docs, sign-in and sign-up as public', () => {
    for (const path of ['/', '/docs', '/login', '/signup']) expect(isPublicPage(path)).toBe(true);
  });

  it('keeps the console and the live view private, at any depth', () => {
    for (const path of ['/dashboard', '/dashboard/keys', '/live', '/live/oya-123'])
      expect(isPublicPage(path)).toBe(false);
  });

  it('matches a private prefix only as a whole segment', () => {
    expect(isPublicPage('/dashboards')).toBe(true);
    expect(isPublicPage('/lively')).toBe(true);
  });
});
