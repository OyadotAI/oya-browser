/** Repository for explicitly supplied workflow files: bounded snapshots, exact metadata, no renderer paths. */
import { open, realpath } from 'node:fs/promises';
import { constants, type Stats } from 'node:fs';
import path from 'node:path';
import mime from 'mime-types';
import type { Draft } from '../../workflow/index.ts';
import { nativeValue } from './native-preflight.ts';
import { NATIVE_VALIDATION } from './constants.ts';
/** Open descriptor belongs only to this repository. */
type Handle = Awaited<ReturnType<typeof open>>;
/** Authorized immutable file bytes and native File metadata. */
export interface WorkflowFile {
  /** Base name only; no local path enters the page. */ name: string;
  /** MIME type, empty when unknown. */ type: string;
  /** Millisecond file modification timestamp. */ lastModified: number;
  /** Exact byte length of the immutable snapshot. */ size: number;
  /** Bounded content snapshot. */ base64: string;
}
/** Snapshot before any browser side effect so a later missing/oversized file cannot cause a partial run. */
export async function workflowFiles(draft: Draft, vars: Record<string, unknown>): Promise<Map<string, WorkflowFile[]>> {
  const files = new Map<string, WorkflowFile[]>(),
    repository = new FileSnapshots();
  for (const step of draft.steps.filter((s) => s.enabled && s.action === 'upload_file')) {
    const value = nativeValue(step.file, vars);
    files.set(step.id, value ? [await repository.read(value)] : []);
  }
  return files;
}
/** Resolve and open only explicit absolute regular files; folders and oversized uploads fail closed. */
async function snapshotFile(value: string, remaining: number): Promise<WorkflowFile> {
  if (!path.isAbsolute(value)) throw Error('Workflow uploads require an explicit absolute file path');
  const filename = await realpath(value),
    handle = await open(filename, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    return await readFileSnapshot(handle, path.basename(value), remaining);
  } finally {
    await handle.close();
  }
}
/** Read at most the declared bound, rejecting files changed during the snapshot. */
async function readFileSnapshot(handle: Handle, name: string, remaining: number): Promise<WorkflowFile> {
  const before = await permittedFile(handle, remaining);
  const bytes = Buffer.alloc(before.size);
  const read = await handle.read(bytes, 0, bytes.length, 0),
    after = await handle.stat();
  if (read.bytesRead !== before.size || after.size !== before.size || after.mtimeMs !== before.mtimeMs)
    throw Error('Workflow upload changed while reading');
  return fileData(name, before, bytes);
}

/** Bound allocations before reading any file bytes. */
async function permittedFile(handle: Awaited<ReturnType<typeof open>>, remaining: number) {
  const info = await handle.stat();
  if (!info.isFile() || info.size > Math.min(NATIVE_VALIDATION.FILE_BYTES, remaining))
    throw Error('Workflow upload is not a regular file within the size limit');
  return info;
}

/** Deduplicate repeated uploads and bound aggregate retained bytes for the entire draft. */
class FileSnapshots {
  /** Native file contents retained across repeated workflow steps. */ private readonly cache = new Map<
    string,
    WorkflowFile
  >();
  /** Total authorized bytes retained in memory. */ private used = 0;
  /** Read each explicit path once and enforce the run budget before allocating its buffer. */
  async read(value: string): Promise<WorkflowFile> {
    const known = this.cache.get(value);
    if (known) return known;
    const file = await snapshotFile(value, NATIVE_VALIDATION.FILE_TOTAL_BYTES - this.used);
    this.used += file.size;
    this.cache.set(value, file);
    return file;
  }
}

/** Only safe File metadata and the bounded byte snapshot enter the renderer. */
function fileData(name: string, info: Stats, bytes: Buffer): WorkflowFile {
  return {
    name,
    type: mime.lookup(name) || '',
    lastModified: info.mtimeMs,
    size: info.size,
    base64: bytes.toString('base64'),
  };
}
