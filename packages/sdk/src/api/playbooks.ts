/**
 * `oya.playbooks`: playbooks saved with `browser.toPlaybook()`, and the
 * drafts older healed replays left behind.
 */
import { segment } from '../client.js';
import type { ImportOptions, Playbook, PlaybookExport, PlaybookSummary } from '../types/index.js';
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
  ...transferApi(http),
});

/** Moving a playbook to another environment: export it here, import it there. */
const transferApi = (http: HttpRef) => ({
  /** The playbook as one JSON document, to move it to another environment with `import()`. Secrets travel by name only. */
  export: async (name: string): Promise<PlaybookExport> =>
    http().request<PlaybookExport>('GET', `${playbook(name)}/export`),
  /**
   * Save an exported playbook here, as `name` or the name it was exported with. A name
   * already taken is refused unless `overwrite` is true.
   */
  import: async (doc: PlaybookExport, options: ImportOptions = {}): Promise<Playbook> =>
    http().request<Playbook>('POST', '/api/playbooks/import', { playbook: doc, ...options }),
});

/** What the server answers for a field. */
interface AnswerReply {
  /** The field's text. */
  answer: string;
}

/** Builds `oya.llm`. */
export const llmApi = (http: HttpRef) => ({
  /**
   * The text for one free-text field, written by this key's model: what an exported
   * playbook calls as `oya.llm.answer(question, vars)` for a comment or a question's answer.
   */
  answer: async (question: string, values: Record<string, unknown> = {}, task?: string): Promise<string> =>
    (await http().request<AnswerReply>('POST', '/api/playbooks/answer', { question, values, task })).answer,
});
