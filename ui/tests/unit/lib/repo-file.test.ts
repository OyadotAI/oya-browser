/** Repository-root files are found whether the site starts from the root or from ui/. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import path from 'node:path';
import { repoFile } from '@/lib/repo-file';

/** The repository root, one level above ui/, where the tests run. */
const ROOT = path.resolve(process.cwd(), '..');

afterEach(() => vi.restoreAllMocks());

describe('repoFile', () => {
  it('finds the file one level up when started from ui/', () => {
    expect(repoFile('CHANGELOG.md')).toBe(path.join(ROOT, 'CHANGELOG.md'));
  });
  it('finds the file in place when started from the repository root', () => {
    vi.spyOn(process, 'cwd').mockReturnValue(ROOT);
    expect(repoFile('compliance', 'evidence.json')).toBe(path.join(ROOT, 'compliance', 'evidence.json'));
  });
});
