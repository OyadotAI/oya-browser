/**
 * Unit tests for file() (src/file.ts): names, MIME types, base64 and the size limit.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { file, MAX_FILE_BYTES } from '../../dist/index.js';

describe('file', () => {
  it('reads a path, names the file after it and guesses the MIME type', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'oya-sdk-'));
    writeFileSync(join(dir, 'cv.PDF'), 'hello');
    const value = await file(join(dir, 'cv.PDF'));
    assert.deepEqual(value, { file: 'cv.PDF', type: 'application/pdf', b64: Buffer.from('hello').toString('base64') });
  });

  it('calls raw bytes "file" with an octet-stream type', async () => {
    assert.deepEqual(await file(new Uint8Array([1, 2, 3])), {
      file: 'file',
      type: 'application/octet-stream',
      b64: 'AQID',
    });
  });

  it("takes a File's own name, and lets options override name and type", async () => {
    const blob = new File(['x'], 'photo.png');
    assert.equal((await file(blob)).type, 'image/png');
    const named = await file(blob, { name: 'a.csv', type: 'text/x-custom' });
    assert.deepEqual([named.file, named.type], ['a.csv', 'text/x-custom']);
  });

  it('refuses a file over the limit, saying how big it is', async () => {
    await assert.rejects(
      file(new Uint8Array(MAX_FILE_BYTES + 1), { name: 'big.bin' }),
      /big\.bin is 10MB; the limit for a task file is 10MB/,
    );
  });

  it('encodes without Buffer the same way (the browser path)', async () => {
    const real = globalThis.Buffer;
    const bytes = new Uint8Array(20_000).map((_, i) => i % 256);
    const expected = real.from(bytes).toString('base64');
    // @ts-expect-error: simulate a browser, where Buffer does not exist
    delete globalThis.Buffer;
    try {
      assert.equal((await file(bytes)).b64, expected);
    } finally {
      globalThis.Buffer = real;
    }
  });
});
