/** The control storage contract over Postgres functions (server/migrations/008_durable_control.sql). */
import { Status } from '../../../platform/http-status.ts';
import { commandsPending, controlPaused, fail } from './errors.ts';
import { DEFAULT_EVENT_LIMIT } from './constants.ts';

/** The error an RPC failure stands for, by its message text; null for a version conflict. */
function rpcFailure(error) {
  const message = String(error.message || '');
  if (message.includes('commands_pending')) return commandsPending();
  if (message.includes('control_paused')) return controlPaused();
  if (message.includes('control_conflict')) return null;
  return Object.assign(fail('storage_unavailable', 'Control storage unavailable', Status.UNAVAILABLE), {
    cause: error,
  });
}

/** The same contract over Postgres functions (server/migrations/008_durable_control.sql). */
export class RemoteBackend {
  /** Client with an rpc(name, args): the Postgres adapter from pg-client. */
  declare client: any;
  constructor(client) {
    this.client = client;
  }
  /** Run an RPC, turning its error text back into the same coded errors the SQLite backend throws; `{ conflict: true }` on a version conflict. */
  async call(name, args) {
    const { data, error } = await this.client.rpc(name, args);
    if (!error) return { data };
    const failure = rpcFailure(error);
    if (failure) throw failure;
    return { conflict: true };
  }
  /** Rows for each query, via control_load. */
  async load(queries) {
    return (await this.call('control_load', { queries })).data;
  }
  /** Commit writes and events via control_commit; `{ ok: false }` on a version conflict. */
  async commit({ writes = [], events = [] }) {
    const { data, conflict } = await this.call('control_commit', { writes, events });
    return conflict ? { ok: false } : data;
  }
  /** Events for a project after a sequence number, the latest ones, or specific `seqs`. */
  async events({ project = null, after = 0, limit = DEFAULT_EVENT_LIMIT, latest = false, seqs = null } = {}) {
    return (
      await this.call('control_read_events', { target_project: project, after_seq: after, lim: limit, latest, seqs })
    ).data;
  }
  /** Delete expired rows and events older than each project's cutoff. */
  async prune(now, cutoffs = {}) {
    await this.call('control_prune', { now_ms: now, cutoffs });
  }
  /** Admit an agent command against the session gate; returns its fence. */
  async beginCommand(id, holder, owner) {
    return (await this.call('control_begin', { session_id: id, actor: holder, caller_instance: owner })).data;
  }
  /** Settle a command admitted under `fence`. */
  async finishCommand(id, fence) {
    if (fence !== null) await this.call('control_finish', { session_id: id, generation: fence });
  }
  /** Nothing to close: the RPC client is shared. */
  close() {}
}
