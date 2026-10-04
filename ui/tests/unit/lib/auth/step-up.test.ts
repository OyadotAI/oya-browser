/**
 * Unit tests for the way to the two-factor code page and back: the link that
 * carries where to return, and the return address only ever being this site.
 */
import { describe, it, expect } from 'vitest';
import { MFA_PAGE, onMfaPage, safeNext, stepUpUrl } from '@/lib/auth/step-up';

describe('stepUpUrl', () => {
  it('links to the code page with where to come back to', () => {
    expect(stepUpUrl('/admin?tab=fleet')).toBe('/account/mfa?next=%2Fadmin%3Ftab%3Dfleet');
    expect(onMfaPage(MFA_PAGE)).toBe(true);
    expect(onMfaPage('/admin')).toBe(false);
  });
});

describe('safeNext', () => {
  it('keeps a path on this site, with its query and fragment', () => {
    expect(safeNext('/admin?tab=fleet#top')).toBe('/admin?tab=fleet#top');
  });

  it('refuses nothing at all', () => {
    expect(safeNext(null)).toBe('');
    expect(safeNext('')).toBe('');
  });

  it('refuses an absolute URL', () => {
    expect(safeNext('https://evil.example/admin')).toBe('');
  });

  it('refuses a protocol-relative URL', () => {
    expect(safeNext('//evil.example/admin')).toBe('');
  });

  it('refuses a backslash, which browsers read as a slash', () => {
    expect(safeNext('/\\evil.example')).toBe('');
  });

  it('refuses a tab or newline, which browsers strip into //host', () => {
    expect(safeNext('/\t/evil.example')).toBe('');
    expect(safeNext('/\n/evil.example')).toBe('');
  });
});
