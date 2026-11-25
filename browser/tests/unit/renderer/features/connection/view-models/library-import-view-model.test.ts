/** Explicit profile consent, duplicate suppression and retry for local-only library imports. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { LibraryImportViewModel } from '../../../../../../src/renderer/features/connection/view-models/library-import-view-model.ts';
import { fakeBridge } from '../../../support/bridge.ts';
import { settle } from '../harness.ts';
const sources = [
  {
    id: 'edge',
    name: 'Edge',
    sessions: false,
    profiles: [
      { id: 'Default', name: 'Home' },
      { id: 'Profile 1', name: 'Work' },
    ],
  },
];
it('does not read a profile before an explicit selection and reports accepted partial counts', async () => {
  const fake = fakeBridge({
    libraryImportSources: sources,
    importLibrary: { bookmarks: 2, history: 0, warnings: ['History could not be read.'] },
  });
  const vm = new LibraryImportViewModel(fake.bridge);
  await settle();
  await vm.start();
  assert.equal(fake.called('importLibrary').length, 0);
  vm.chooseProfile('Profile 1');
  await vm.start();
  assert.deepEqual(fake.called('importLibrary'), [['edge', 'Profile 1']]);
  assert.match(vm.state.note, /2 bookmarks and 0 history entries.*History could not be read/);
});
it('distinguishes discovery failure and permits a read retry without exposing raw errors', async () => {
  let fail = true;
  const fake = fakeBridge({
    libraryImportSources: sources,
    importLibrary: () => {
      if (fail) throw Error('C:\\private\\profile');
      return { bookmarks: 1, history: 0, warnings: [] };
    },
  });
  const vm = new LibraryImportViewModel(fake.bridge);
  await settle();
  vm.chooseProfile('Default');
  await vm.start();
  assert.match(vm.state.error, /Could not read/);
  assert(!vm.state.error.includes('private'));
  fail = false;
  await vm.start();
  assert.equal(vm.state.error, '');
  assert.match(vm.state.note, /1 bookmarks/);
});
