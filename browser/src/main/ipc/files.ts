/**
 * Files the person asked to save (diagnostics, Playwright exports, workflows):
 * written owner-only, since each can carry what a page showed.
 */
import fs from 'node:fs';
import { PRIVATE_FILE_MODE } from '../app/constants.ts';

/** Writes `text` to `file`, owner-only, synchronously. */
export function writePrivateFileSync(file: string, text: string): void {
  fs.writeFileSync(file, text, { mode: PRIVATE_FILE_MODE });
}

/** Writes `text` to `file`, owner-only. */
export function writePrivateFile(file: string, text: string): Promise<void> {
  return fs.promises.writeFile(file, text, { mode: PRIVATE_FILE_MODE });
}
