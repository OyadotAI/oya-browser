/** Authorized upload snapshots are bounded, immutable and retain correct browser File metadata. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm, truncate } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { workflowFiles } from '../../../../src/main/workflow/native-files.ts';
import { normalizeDraft } from '../../../../src/workflow/index.ts';
import { NATIVE_VALIDATION } from '../../../../src/main/workflow/constants.ts';
/** File upload draft with an exact explicit path. */
const draft = (file: string) =>
  normalizeDraft({
    steps: [{ id: 'upload', action: 'upload_file', file, candidates: [{ kind: 'css', value: '#file' }] }],
  });
it('snapshots explicit files with MIME, basename and stable bytes before execution', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'oya-workflow-file-'));
  try {
    const file = path.join(dir, 'example.txt');
    await writeFile(file, 'authorized bytes');
    const result = await workflowFiles(draft(file), {});
    await writeFile(file, 'changed later');
    const saved = result.get('upload')![0];
    assert.equal(saved.name, 'example.txt');
    assert.equal(saved.type, 'text/plain');
    assert.equal(Buffer.from(saved.base64, 'base64').toString(), 'authorized bytes');
    assert.equal(saved.size, 16);
    assert.ok(saved.lastModified > 0);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
it('refuses relative paths, directories, missing and oversized files before browser use', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'oya-workflow-file-'));
  try {
    await assert.rejects(workflowFiles(draft('relative.txt'), {}), /absolute file path/);
    await assert.rejects(workflowFiles(draft(dir), {}), /regular file/);
    await assert.rejects(workflowFiles(draft(path.join(dir, 'missing')), {}), /ENOENT/);
    const file = path.join(dir, 'huge');
    await writeFile(file, '');
    await truncate(file, NATIVE_VALIDATION.FILE_BYTES + 1);
    await assert.rejects(workflowFiles(draft(file), {}), /size limit/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
it('empty file assignment explicitly clears the input without reading local files', async () => {
  const files = await workflowFiles(draft(''), {});
  assert.deepEqual(files.get('upload'), []);
});
