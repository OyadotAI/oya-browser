/** Local import validates source/profile boundaries and preserves the selected Oya partition. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { LibraryImport } from '../../../../src/main/mirror/library-import.ts';
import { BrowsingLibrary } from '../../../../src/main/library/index.ts';

/** A scratch source and in-memory destination exercise the public importer contract. */
function fixture(t: any) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-library-import-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const profile of ['Default', 'Profile 1']) fs.mkdirSync(path.join(root, profile));
  let partition = 'current';
  const values = {};
  const deps = {
    config: { values, merge: (patch: object) => Object.assign(values, patch), save() {} },
    persona: { partitionName: () => partition },
  };
  const library = new BrowsingLibrary(deps as any);
  const source = {
    id: 'edge',
    name: 'Edge',
    kind: 'chromium' as const,
    exe: '',
    userDataDir: root,
    profiles: [
      { dir: 'Default', name: 'Home', lastUsed: true },
      { dir: 'Profile 1', name: 'Work', lastUsed: false },
    ],
  };
  const read = () => ({
    bookmarks: [
      { url: 'https://example.com/', title: 'Example', time: 1 },
      { url: 'https://example.com/', title: 'Duplicate', time: 1 },
      { url: 'file:///private', title: 'Unsafe', time: 1 },
    ],
    history: [],
    warnings: [] as string[],
  });
  const seams = { sources: () => [source], realpath: fs.realpathSync, read };
  const importer = new LibraryImport({ library, persona: deps.persona } as any, seams);
  return {
    root,
    source,
    seams,
    importer,
    library,
    changePartition: (next: string) => {
      partition = next;
    },
  };
}
it('lists two named profiles without an executable or any filesystem path in the result', (t) => {
  const { importer, root } = fixture(t);
  const choices = importer.sources();
  assert.deepEqual(
    choices[0].profiles.map((p) => p.name),
    ['Home', 'Work'],
  );
  assert(!JSON.stringify(choices).includes(root));
});
it('counts only safe unique additions and makes repeated imports idempotent', (t) => {
  const { importer, library } = fixture(t);
  assert.deepEqual(importer.import('edge', 'Profile 1'), { bookmarks: 1, history: 0, warnings: [] });
  assert.deepEqual(importer.import('edge', 'Profile 1'), { bookmarks: 0, history: 0, warnings: [] });
  assert.equal(library.snapshot().bookmarks.length, 1);
});
it('rejects stale source IDs, traversal and symlink profiles before reading', (t) => {
  const { root, source, importer, seams } = fixture(t);
  const outside = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-outside-import-'));
  t.after(() => fs.rmSync(outside, { recursive: true, force: true }));
  fs.symlinkSync(outside, path.join(root, 'Linked'), 'dir');
  source.profiles.push(
    { dir: '../escape', name: 'Escape', lastUsed: false },
    { dir: 'Linked', name: 'Linked', lastUsed: false },
  );
  let reads = 0;
  seams.read = () => {
    reads++;
    throw Error('Must not read');
  };
  for (const [browser, profile] of [
    ['unknown', 'Default'],
    ['edge', '../escape'],
    ['edge', 'Linked'],
    ['edge', 'C:\\private'],
  ])
    assert.throws(() => importer.import(browser, profile), /available browser profile/);
  assert.equal(reads, 0);
});
it('retains the destination snapshot and partial warnings while leaving other partitions alone', (t) => {
  const { seams, importer, library, changePartition } = fixture(t);
  const original = seams.read;
  seams.read = () => {
    changePartition('other');
    return { ...original(), warnings: ['History could not be read.'] };
  };
  const result = importer.import('edge', 'Default');
  assert.equal(result.bookmarks, 1);
  assert.deepEqual(result.warnings, ['History could not be read.']);
  assert.equal(library.snapshot('current').bookmarks.length, 1);
  assert.equal(library.snapshot('other').bookmarks.length, 0);
});
it('fails an entirely unreadable profile and permits a subsequent retry', (t) => {
  const { seams, importer } = fixture(t);
  const original = seams.read;
  seams.read = () => ({ bookmarks: [], history: [], warnings: ['Bookmarks could not be read.'] });
  assert.throws(() => importer.import('edge', 'Default'), /could not be read/);
  seams.read = original;
  assert.equal(importer.import('edge', 'Default').bookmarks, 1);
});
