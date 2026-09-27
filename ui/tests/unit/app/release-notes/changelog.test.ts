/**
 * Unit tests for reading CHANGELOG.md into releases for the release notes page.
 */
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { inlineOf, parseChangelog } from '@/app/release-notes/changelog';

const SAMPLE = `# Changelog

Intro that is not a release.

## Unreleased

A note.

### Added

- **Bold** thing with \`code\` and a [link](https://x.test).
- Second.

## 1.0.1 · 2026-01-01

### Fixed

- One fix.
`;

describe('parseChangelog', () => {
  it('reads releases, their notes, groups and items, leaving out the intro', () => {
    const releases = parseChangelog(SAMPLE);
    expect(releases.map((r) => r.title)).toEqual(['Unreleased', '1.0.1 · 2026-01-01']);
    expect(releases[0].notes).toEqual(['A note.']);
    expect(releases[0].groups).toEqual([
      { title: 'Added', items: ['**Bold** thing with `code` and a [link](https://x.test).', 'Second.'] },
    ]);
    expect(releases[1].groups[0]).toEqual({ title: 'Fixed', items: ['One fix.'] });
  });

  it('reads the repository changelog into at least one release with changes', () => {
    const file = readFileSync(path.join(process.cwd(), '..', 'CHANGELOG.md'), 'utf8');
    const releases = parseChangelog(file);
    expect(releases.length).toBeGreaterThan(0);
    expect(releases.some((r) => r.groups.some((g) => g.items.length))).toBe(true);
  });
});

describe('inlineOf', () => {
  it('splits a line into text, bold, code and links in order', () => {
    expect(inlineOf('**Bold** then `code` and [a](https://x.test).')).toEqual([
      { kind: 'bold', text: 'Bold' },
      { kind: 'text', text: ' then ' },
      { kind: 'code', text: 'code' },
      { kind: 'text', text: ' and ' },
      { kind: 'link', text: 'a', href: 'https://x.test' },
      { kind: 'text', text: '.' },
    ]);
  });

  it('keeps a plain line whole', () => {
    expect(inlineOf('Just words')).toEqual([{ kind: 'text', text: 'Just words' }]);
  });
});
