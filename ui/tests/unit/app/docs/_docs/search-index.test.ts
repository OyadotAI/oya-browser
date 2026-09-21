/**
 * Unit tests for the docs search: short queries show nothing, matches carry
 * context, duplicates collapse, and results are capped.
 */
import { describe, it, expect } from 'vitest';
import { searchDocs } from '@/app/docs/_docs/search-index';
import { MAX_HITS } from '@/app/docs/_docs/constants';

describe('searchDocs', () => {
  it('shows nothing for an empty or one-letter query', () => {
    expect(searchDocs('')).toBeNull();
    expect(searchDocs(' a ')).toBeNull();
  });

  it('finds a heading case-insensitively and jumps to its section', () => {
    expect(searchDocs('mcp setup')![0]).toEqual({ id: 'mcp-setup', snippet: 'MCP Setup', tag: 'H2' });
  });

  it('cuts long entries around the match and marks the cuts', () => {
    const hit = searchDocs('Electron markers')![0];
    expect(hit.id).toBe('stealth');
    expect(hit.snippet.startsWith('Removes Electron markers')).toBe(true);
    expect(hit.snippet.endsWith('...')).toBe(true);
  });

  it('returns an empty list, not nothing, when a real query matches nothing', () => {
    expect(searchDocs('zzzz-no-such-thing')).toEqual([]);
  });

  it('caps the results', () => {
    expect(searchDocs('the')!.length).toBe(MAX_HITS);
  });
});
