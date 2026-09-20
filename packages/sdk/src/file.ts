/**
 * Files as task values.
 *
 *   await browser.ask('Attach my resume', { data: { resume: await file('./cv.pdf') } });
 *
 * The bytes ride inline in the run request, so the agent's `upload_file` tool can put
 * them into a page's file input. `secrets` cannot hold one — a file is never typed
 * through a placeholder, so there is nothing to hide.
 */

import type { FileValue } from './types/index.js';
import { BASE64_CHUNK_BYTES, BYTES_PER_MB, MAX_FILE_MB } from './constants.js';

/** Bigger than this and the server would refuse the body anyway. */
export const MAX_FILE_BYTES = MAX_FILE_MB * BYTES_PER_MB;

/**
 * Not a literal: bundlers resolve a literal specifier even inside a dynamic import, and
 * esbuild drops the `node:` prefix that would have marked it a builtin — so a browser
 * build would fail on a branch it never runs. Read through a variable, nobody tries.
 */
const NODE_FS = 'node:fs/promises';

/** Enough to keep a site's accept="" filter happy. Anything else is octet-stream. */
const MIME: Record<string, string> = {
  pdf: 'application/pdf',
  png: 'image/png',
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
  gif: 'image/gif',
  webp: 'image/webp',
  svg: 'image/svg+xml',
  heic: 'image/heic',
  txt: 'text/plain',
  csv: 'text/csv',
  json: 'application/json',
  xml: 'application/xml',
  html: 'text/html',
  doc: 'application/msword',
  docx: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  xls: 'application/vnd.ms-excel',
  xlsx: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  ppt: 'application/vnd.ms-powerpoint',
  pptx: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
  zip: 'application/zip',
};

/** Where `file()` got its bytes, and the name the source itself suggests. */
interface Source {
  /** The file's contents. */
  bytes: Uint8Array;
  /** The name a path or a File carries, if any. */
  name?: string;
}

/** The global scope, which has Buffer in Node. */
interface MaybeNode {
  /** Node's Buffer; absent in a browser. */
  Buffer?: BufferLike;
}

/** Node's Buffer, where there is one. */
interface BufferLike {
  /** Wraps bytes so they can be encoded. */
  from(b: Uint8Array): { toString(enc: string): string };
}

/** Base64 through Buffer in Node, through btoa in a browser. */
const base64 = (bytes: Uint8Array): string => {
  const buffer = (globalThis as MaybeNode).Buffer;
  if (buffer) return buffer.from(bytes).toString('base64');
  // btoa takes a string, and spreading 10MB into one apply() call blows the stack.
  let binary = '';
  for (let i = 0; i < bytes.length; i += BASE64_CHUNK_BYTES) {
    binary += String.fromCharCode(...bytes.subarray(i, i + BASE64_CHUNK_BYTES));
  }
  return btoa(binary);
};

/** A path only means something in Node, so this is the one branch that needs fs. */
async function readPath(path: string): Promise<Source> {
  const fs = (await import(/* webpackIgnore: true */ /* @vite-ignore */ NODE_FS)) as {
    readFile(path: string): Promise<Uint8Array>;
  };
  return { bytes: new Uint8Array(await fs.readFile(path)), name: path.split(/[\\/]/).pop() || 'file' };
}

/** The bytes and suggested name of whatever `file()` was given. */
async function readSource(source: string | Uint8Array | Blob): Promise<Source> {
  if (typeof source === 'string') return readPath(source);
  if (source instanceof Uint8Array) return { bytes: source };
  return { bytes: new Uint8Array(await source.arrayBuffer()), name: (source as File).name };
}

/** Refuses a file the server would reject, saying how big it is. */
function checkSize(name: string, bytes: Uint8Array): void {
  if (bytes.length <= MAX_FILE_BYTES) return;
  throw new Error(
    `${name} is ${Math.round(bytes.length / BYTES_PER_MB)}MB; the limit for a task file is ${MAX_FILE_BYTES / BYTES_PER_MB}MB.`,
  );
}

/** The MIME type for a filename's extension, or octet-stream. */
function mimeFor(name: string): string {
  const ext = name.includes('.') ? name.split('.').pop()!.toLowerCase() : '';
  return MIME[ext] || 'application/octet-stream';
}

/** The task value for these bytes, once they are known to fit. */
function fileValue(name: string, bytes: Uint8Array, type: string | undefined): FileValue {
  checkSize(name, bytes);
  return { file: name, type: type || mimeFor(name), b64: base64(bytes) };
}

/**
 * A file for `data`. A string is a path on disk (Node only); a Blob, a File or raw bytes
 * work anywhere. `name` is what the site sees, and `type` overrides the MIME guessed
 * from the extension.
 */
export async function file(
  source: string | Uint8Array | Blob,
  options: {
    /** The filename the site sees. */
    name?: string;
    /** The MIME type, instead of the one guessed from the extension. */
    type?: string;
  } = {},
): Promise<FileValue> {
  const read = await readSource(source);
  return fileValue(options.name || read.name || 'file', read.bytes, options.type);
}
