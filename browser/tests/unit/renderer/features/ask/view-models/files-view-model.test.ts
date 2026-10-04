/**
 * Unit tests for the Ask pane's attachments: pending files become the next
 * message's, then are sent with every turn; the size cap and an unreadable
 * file stop a pick with a note; Clear forgets them all.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { FilesViewModel } from '../../../../../../src/renderer/features/ask/view-models/files-view-model.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';
import { fakeRead, file } from '../support.ts';

describe('FilesViewModel', () => {
  it('names the attached files in the message, and counts them as sent from then on', async () => {
    const files = new FilesViewModel(fakeRead);
    await files.add([file('a.txt', 'YQ=='), file('b.bin', 'Yg==', '')]);
    assert.equal(files.attachTo('go'), 'go\n\n(Attached: a.txt, b.bin)');
    assert.deepEqual(files.state.pending, []);
    assert.deepEqual(files.data(), {
      file1: { file: 'a.txt', type: 'text/plain', b64: 'YQ==' },
      file2: { file: 'b.bin', type: 'application/octet-stream', b64: 'Yg==' },
    });
    assert.equal(files.attachTo('again'), 'again', 'a later message names no files');
  });

  it('refuses a file past the size cap, with a note, keeping the ones before it', async () => {
    const files = new FilesViewModel(fakeRead);
    await files.add([file('a.txt', 'YQ=='), file('big.iso', 'A'.repeat(C.CHAT_FILES_MAX_B64))]);
    assert.match(files.state.note, /up to 10 MB/);
    assert.deepEqual(
      files.state.pending.map((f) => f.file),
      ['a.txt'],
    );
  });

  it('counts files already sent toward the cap', async () => {
    const files = new FilesViewModel(fakeRead);
    await files.add([file('a.txt', 'A'.repeat(C.CHAT_FILES_MAX_B64 - 1))]);
    files.attachTo('go');
    await files.add([file('b.txt', 'AA')]);
    assert.match(files.state.note, /up to 10 MB/);
  });

  it('says so when a file cannot be read', async () => {
    const files = new FilesViewModel(() => Promise.reject(new Error('gone')));
    await files.add([file('a.txt', '')]);
    assert.equal(files.state.note, 'Could not read that file.');
    assert.deepEqual(files.state.pending, []);
  });

  it('lets a pending file be removed, and the note go with it', async () => {
    const files = new FilesViewModel(fakeRead);
    await files.add([file('a.txt', 'YQ=='), file('big.iso', 'A'.repeat(C.CHAT_FILES_MAX_B64))]);
    files.remove(0);
    assert.deepEqual(files.state, { pending: [], note: '' });
  });

  it('forgets every file on Clear, and sends no data after', async () => {
    const files = new FilesViewModel(fakeRead);
    await files.add([file('a.txt', 'YQ==')]);
    files.attachTo('go');
    files.clear();
    assert.equal(files.data(), undefined);
  });
});
