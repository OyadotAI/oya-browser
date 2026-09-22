/**
 * `oya.playbooks`: playbooks saved with `browser.toPlaybook()`, and the
 * drafts healed replays leave behind.
 */
import { segment } from '../client.js';
import type { Playbook, PlaybookSummary } from '../types/index.js';
import type { HttpRef, PlaybookList } from './shapes.js';

/** A playbook's endpoint. */
const playbook = (name: string) => `/api/playbooks/${segment(name, 'name')}`;

/** Builds `oya.playbooks`. */
export const playbookApi = (http: HttpRef) => ({
  /** Every saved playbook, with any draft waiting on it. */
  list: async (): Promise<PlaybookSummary[]> => (await http().request<PlaybookList>('GET', '/api/playbooks')).playbooks,
  /** Delete a playbook and its draft, or only the draft with `'<name>:draft'`. */
  remove: async (name: string): Promise<void> => {
    await http().request('DELETE', playbook(name));
  },
  /** Replace a playbook with the draft a healed replay saved. Try it first with `browser.play('<name>:draft')`. */
  promote: async (name: string): Promise<Playbook> => http().request<Playbook>('POST', `${playbook(name)}/promote`, {}),
});
