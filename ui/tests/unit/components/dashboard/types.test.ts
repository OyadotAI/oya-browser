/**
 * Unit tests for the label helpers: known ids get names, unknown ones show as
 * themselves, missing ones get a placeholder.
 */
import { describe, it, expect } from 'vitest';
import { platformLabel, providerLabel } from '@/components/dashboard/types';

describe('providerLabel', () => {
  it('names a known provider', () => expect(providerLabel('oya-desktop')).toBe('Desktop'));
  it('shows an unknown provider as its id', () => expect(providerLabel('acme')).toBe('acme'));
  it('shows a dash for no provider', () => expect(providerLabel(null)).toBe('—'));
});

describe('platformLabel', () => {
  it('names a known platform', () => expect(platformLabel('MacIntel')).toBe('macOS'));
  it('shows an unknown platform as itself', () => expect(platformLabel('FreeBSD')).toBe('FreeBSD'));
  it('reads a missing platform as auto', () => expect(platformLabel(undefined)).toBe('auto'));
});
