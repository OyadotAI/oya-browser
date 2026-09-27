/**
 * Moving a playbook between environments from the browser: its export saved as a
 * file, and a file read back for import.
 */
import { EXPORT_INDENT } from './constants';

/** Saves `doc` as `<name>.oya-playbook.json` through the browser's download. */
export function downloadExport(name: string, doc: unknown) {
  const blob = new Blob([JSON.stringify(doc, null, EXPORT_INDENT)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = `${name}.oya-playbook.json`;
  link.click();
  URL.revokeObjectURL(url);
}

/** The JSON in a chosen file; a file that is not JSON says so rather than sending garbage. */
export async function readExport(file: File): Promise<unknown> {
  try {
    return JSON.parse(await file.text());
  } catch {
    throw new Error(`${file.name} is not a playbook export (not JSON)`);
  }
}
