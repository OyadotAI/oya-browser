/**
 * Unit tests for the table formatters: relative times and short ids.
 */
import { describe, it, expect } from 'vitest';
import { ago, shortId } from '@/lib/format';

const NOW = Date.parse('2026-01-01T12:00:00Z');
/** An ISO time `sec` seconds before NOW. */
const before = (sec: number) => new Date(NOW - sec * 1000).toISOString();

describe('ago', () => {
  it('shows a dash for no time', () => {
    expect(ago(undefined, NOW)).toBe('—');
    expect(ago('', NOW)).toBe('—');
  });

  it('reads as now for the first few seconds, and for times in the future', () => {
    expect(ago(before(4), NOW)).toBe('now');
    expect(ago(before(-60), NOW)).toBe('now');
  });

  it('counts seconds, minutes, hours with minutes, then days', () => {
    expect(ago(before(5), NOW)).toBe('5s');
    expect(ago(before(59), NOW)).toBe('59s');
    expect(ago(before(60), NOW)).toBe('1m');
    expect(ago(before(3599), NOW)).toBe('59m');
    expect(ago(before(3600 + 12 * 60), NOW)).toBe('1h 12m');
    expect(ago(before(86_400 * 3), NOW)).toBe('3d');
  });
});

describe('shortId', () => {
  it('keeps ids up to 14 characters and shortens longer ones', () => {
    expect(shortId('a'.repeat(14))).toBe('a'.repeat(14));
    expect(shortId('abcdefghijklmnop')).toBe('abcdefgh…');
  });
});
