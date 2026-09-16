/**
 * Files as task values.
 *
 *   await browser.ask('Attach my resume', { data: { resume: await file('./cv.pdf') } });
 *
 * The bytes ride inline in the run request, so the agent's `upload_file` tool can put
 * them into a page's file input. `secrets` cannot hold one — a file is never typed
 * through a placeholder, so there is nothing to hide.
 */

import type { FileValue } from './types.js';

/** Bigger than this and the server would refuse the body anyway. */
export const MAX_FILE_BYTES = 10 * 1024 * 1024;

/**
 * Not a literal: bundlers resolve a literal specifier even inside a dynamic import, and
 * esbuild drops the `node:` prefix that would have marked it a builtin — so a browser
 * build would fail on a branch it never runs. Read through a variable, nobody tries.
 */
const NODE_FS = 'node:fs/promises';

/** Enough to keep a site's accept="" filter happy. Anything else is octet-stream. */
const MIME: Record<string, string> = {
  pdf: 'application/pdf',
  png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', gif: 'image/gif', webp: 'image/webp', svg: 'image/svg+xml', heic: 'image/heic',
  txt: 'text/plain', csv: 'text/csv', json: 'application/json', xml: 'application/xml', html: 'text/html',
  doc: 'application/msword',
  docx: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
  xls: 'application/vnd.ms-excel',
  xlsx: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  ppt: 'application/vnd.ms-powerpoint',
  pptx: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
  zip: 'application/zip',
};

const base64 = (bytes: Uint8Array): string => {
  const buffer = (globalThis as { Buffer?: { from(b: Uint8Array): { toString(enc: string): string } } }).Buffer;
  if (buffer) return buffer.from(bytes).toString('base64');
  // btoa takes a string, and spreading 10MB into one apply() call blows the stack.
  let binary = '';
  for (let i = 0; i < bytes.length; i += 8192) binary += String.fromCharCode(...bytes.subarray(i, i + 8192));
  return btoa(binary);
};

/**
 * A file for `data`. A string is a path on disk (Node only); a Blob, a File or raw bytes
 * work anywhere. `name` is what the site sees, and `type` overrides the MIME guessed
 * from the extension.
 */
export async function file(
  source: string | Uint8Array | Blob,
  options: { name?: string; type?: string } = {},
): Promise<FileValue> {
  let bytes: Uint8Array;
  let name = options.name;

  if (typeof source === 'string') {
    // A path only means something in Node, so this is the one branch that needs fs.
    const fs = await import(/* webpackIgnore: true */ /* @vite-ignore */ NODE_FS) as {
      readFile(path: string): Promise<Uint8Array>;
    };
    bytes = new Uint8Array(await fs.readFile(source));
    name ||= source.split(/[\\/]/).pop() || 'file';
  } else if (source instanceof Uint8Array) {
    bytes = source;
  } else {
    bytes = new Uint8Array(await source.arrayBuffer());
    name ||= (source as { name?: string }).name;
  }
  name ||= 'file';

  if (bytes.length > MAX_FILE_BYTES) {
    throw new Error(`${name} is ${Math.round(bytes.length / 1024 / 1024)}MB; the limit for a task file is ${MAX_FILE_BYTES / 1024 / 1024}MB.`);
  }

  const ext = name.includes('.') ? name.split('.').pop()!.toLowerCase() : '';
  return { file: name, type: options.type || MIME[ext] || 'application/octet-stream', b64: base64(bytes) };
}
