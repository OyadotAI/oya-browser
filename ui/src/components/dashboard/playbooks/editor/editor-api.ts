/**
 * The requests the playbook editor makes: read a playbook's whole definition,
 * save an edit over it, save it under a new name, and test an unsaved edit on a
 * browser. The server checks every edit as it checks an import, and runs no code.
 */
import { api } from '@/lib/api-client';
import type { PlaybookBody, RunInfo } from '../types';

/** What an import takes: the export layout the server reads (transfer.ts EXPORT_FORMAT and EXPORT_VERSION). */
const EXPORT_FORMAT = 'oya-playbook';
/** The export layout's version. */
const EXPORT_VERSION = 1;

/** A path segment, escaped. */
const seg = encodeURIComponent;

/** A playbook's definition: its steps, defaults, secret names and, for a workflow, its variables. */
export type Definition = Record<string, unknown> & {
  /** The steps, as recorded or edited. */
  steps?: unknown[];
  /** Default values by variable name. */
  defaults?: Record<string, string>;
  /** The variables that hold secrets. */
  secrets?: string[];
  /** 2 for a workflow, whose variables carry their own defaults and secret flags. */
  schemaVersion?: number;
  /** A workflow's variables. */
  variables?: Record<string, WorkflowVariable>;
};

/** One variable of a workflow. */
export interface WorkflowVariable {
  /** Whether it holds a secret. */
  secret?: boolean;
  /** Its default value. */
  default?: string;
}

/** A run as the tester follows it: the replay's steps as they end. */
export type TestRun = RunInfo & {
  /** Each replayed step, in order. */
  events?: StepEvent[];
};

/** One replayed step. */
export interface StepEvent {
  /** Its place, from 0. */
  index: number;
  /** What it does. */
  action: string;
  /** Whether it played as recorded. */
  ok: boolean;
}

/** The playbook's whole definition; secret defaults are never sent back. */
export const getDefinition = (key: string, name: string) => api<Definition>(`/playbooks/${seg(name)}`, { key });

/** Saves an edited definition over the playbook. */
export const saveDefinition = (key: string, name: string, definition: Definition) =>
  api<PlaybookBody>(`/playbooks/${seg(name)}`, { key, method: 'PUT', body: definition });

/** Saves the definition as a new playbook of another name; 409 when that name is taken. */
export const saveDefinitionAs = (key: string, name: string, definition: Definition) =>
  api<PlaybookBody>('/playbooks/import', {
    key,
    method: 'POST',
    body: { playbook: { format: EXPORT_FORMAT, version: EXPORT_VERSION, playbook: { ...definition, name } }, name },
  });

/** Replays an unsaved definition on a browser, strictly unless `autoHeal`. */
export const testDefinition = (key: string, browserId: string, body: TestBody) =>
  api<TestRun>(`/browsers/${seg(browserId)}/runs`, { key, method: 'POST', body });

/** What a test run sends. */
export interface TestBody {
  /** The unsaved definition. */
  playbook: Definition;
  /** Sample values for its variables. */
  data: Record<string, string>;
  /** Let the agent finish a step that broke. */
  autoHeal: boolean;
}
