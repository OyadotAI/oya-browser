/** Native file dialogs for moving saved playbooks between projects. */
import fs from 'node:fs';
import type { AppServices } from '../app/services.ts';
import { ServerApi } from '../connection/server-api.ts';
import { ShellDialogs } from './shell-dialogs.ts';
import { writePrivateFile } from './files.ts';
import { MAX_IMPORT_BYTES, SAFE_FILE_NAME } from './constants.ts';
import { JSON_INDENT } from '../app/constants.ts';

/** Services required for server-backed playbook transfers. */
type Deps = Pick<AppServices, 'config' | 'socket' | 'shell' | 'electron'>;
/** File types accepted by the native picker. */
const FILTERS = [{ name: 'Oya playbook', extensions: ['json'] }];

/** Import a selected JSON file without overwriting an existing playbook. */
export async function importPlaybook(deps: Deps): Promise<object> {
  const picked = await new ShellDialogs(deps).open({ properties: ['openFile'], filters: FILTERS });
  const file = picked.filePaths?.[0];
  if (picked.canceled || !file) return { canceled: true };
  if ((await fs.promises.stat(file)).size > MAX_IMPORT_BYTES) throw new Error('This playbook file is too large.');
  const playbook: unknown = JSON.parse(await fs.promises.readFile(file, 'utf8'));
  return (await new ServerApi(deps).post('playbooks/import', { playbook })) as object;
}

/** Export through the server sanitizer, then save only at the chosen location. */
export async function exportPlaybook(deps: Deps, name: string): Promise<object> {
  const body = await new ServerApi(deps).get(`playbooks/${encodeURIComponent(name)}/export`);
  const defaultPath = `${SAFE_FILE_NAME.test(name) ? name : 'playbook'}.json`;
  const picked = await new ShellDialogs(deps).save({ title: 'Export playbook', defaultPath, filters: FILTERS });
  if (picked.canceled) return { canceled: true };
  await writePrivateFile(picked.filePath, JSON.stringify(body, null, JSON_INDENT));
  return { exported: true };
}
