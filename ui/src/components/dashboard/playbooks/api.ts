/**
 * Every request the Playbooks tab makes, in one place: the playbook list and
 * its edits, recording on a live browser, and replay runs.
 */
import { api } from '@/lib/api-client';
import type { PlaybookBody, PlaybookList, RecordedStep, RunInfo, StartedBrowser } from './types';

/** A path segment, escaped. */
const seg = encodeURIComponent;

/** What POST /control/sessions/:id/input answers. */
export interface InputResult {
  /** Whether the browser did it. */
  ok: boolean;
  /** Why not. */
  error?: string;
}

/** What a recording request asks for. */
export interface RecordRequest {
  /** start, stop, status or discard. */
  mode: string;
  /** Hand control back after stopping. */
  resume?: boolean;
}

/** Stops capture and hands control back. */
export const STOP_AND_RESUME: RecordRequest = { mode: 'stop', resume: true };

/** The saved playbooks. */
export const listPlaybooks = (key: string) => api<PlaybookList>('/playbooks', { key });

/** Replaces a playbook's steps with its healed draft. */
export const promotePlaybook = (key: string, name: string) =>
  api(`/playbooks/${seg(name)}/promote`, { key, method: 'POST', body: {} });

/** Deletes a playbook, or only its draft when `name` ends in `:draft`. */
export const deletePlaybook = (key: string, name: string) => api(`/playbooks/${seg(name)}`, { key, method: 'DELETE' });

/** Renames a playbook; its draft moves with it. */
export const renamePlaybook = (key: string, from: string, name: string) =>
  api(`/playbooks/${seg(from)}`, { key, method: 'PATCH', body: { name } });

/** Starts, stops, reads or discards the recording on a browser. */
export const recordCall = <T = unknown>(key: string, browserId: string, body: RecordRequest) =>
  api<T>(`/control/sessions/${seg(browserId)}/record`, { key, method: 'POST', body });

/** Sends one input action to a browser a person holds. */
export const postInput = (key: string, browserId: string, action: string, params: Record<string, unknown>) =>
  api<InputResult>(`/control/sessions/${seg(browserId)}/input`, { key, method: 'POST', body: { action, params } });

/** Takes (or renews) a person's hold on a browser; `force` takes it from someone else. */
export const acquireControl = (key: string, browserId: string, force = false) =>
  api(`/control/sessions/${seg(browserId)}/control`, { key, method: 'POST', body: { action: 'acquire', force } });

/** Saves recorded steps as a playbook. */
export const savePlaybook = (key: string, browserId: string, body: SaveRequest) =>
  api<PlaybookBody>(`/browsers/${seg(browserId)}/playbooks`, { key, method: 'POST', body });

/** What saving a recording sends. */
export interface SaveRequest {
  /** The playbook's name. */
  name: string;
  /** What the flow does, for healing a broken replay. */
  prompt: string;
  /** The recorded steps. */
  steps: RecordedStep[];
  /** Variables that hold passwords. */
  secrets: string[];
}

/** What starting a run sends. */
export interface RunRequest {
  /** Which playbook. */
  playbook: string;
  /** Variable values. */
  data: Record<string, string>;
  /** Let the agent finish and draft a fix when the page changed. */
  autoHeal: boolean;
}

/** Replays a playbook on a browser. */
export const startRun = (key: string, browserId: string, body: RunRequest) =>
  api<RunInfo>(`/browsers/${seg(browserId)}/runs`, { key, method: 'POST', body });

/** A run's current state. */
export const getRun = (key: string, runId: string) => api<RunInfo>(`/runs/${seg(runId)}`, { key });

/** Answers a run that stopped for a person. */
export const respondToRun = (key: string, runId: string, response: string) =>
  api(`/runs/${seg(runId)}/respond`, { key, method: 'POST', body: { response } });

/** Starts a browser on a profile. */
export const startBrowser = (key: string, persona: string) =>
  api<StartedBrowser>('/browsers/start', { key, method: 'POST', body: { persona } });
