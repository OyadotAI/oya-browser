/**
 * Erasure: a deleted project's data is removed from every place it lives once
 * its grace period is over, and a person can delete their account, which
 * deletes every project they own the same way.
 *
 * Deleting a project only marks it (control/service/projects.ts): its browsers
 * still need its sealed key to be cleaned up. Every replica's maintenance pass
 * calls maintain(). Past the grace period each replica drops what it holds in
 * memory or on its own disk for the project; once that has settled, storage
 * is erased and the project row is cut down to a tombstone, which keeps its
 * key from ever opening a new project. The audit log is kept, and the
 * project's control events age out under the audit retention like any other's.
 */
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { BILLING_STATES, PROJECT_KINDS, PURGE_GRACE_MS, PURGE_SETTLE_MS } from './constants.ts';

/** A project row as erasure reads it. */
type Project = {
  /** The project id. */
  id: string;
  /** The owner fingerprint its personas, settings, usage, profiles and recordings are filed under. */
  legacyOwner: string;
  /** When it was deleted, in ms; absent while it lives. */
  deletedAt?: number;
  /** When its data was erased, in ms; absent until then. */
  purgedAt?: number;
};

/** The storage erasure deletes from, apart from the control plane. */
export type ErasureRows = {
  /** Deletes a project's rows from every table keyed by its owner or id. */
  deleteProjectRows(owner: string, project: string): Promise<void>;
  /** Deletes a person's keys and profile. */
  deleteUserRows(userId: string): Promise<void>;
  /** A person's subscription row, or null. */
  subscriptionOf(userId: string): Promise<Record<string, unknown> | null>;
  /** Deletes the person's sign-in. */
  deleteSignIn(userId: string): Promise<void>;
};

/** What erasure is wired to by the composition root. */
export type ErasureDeps = {
  /** The control-plane store, called late since it is built on first use. */
  store: () => any;
  /** Storage outside the control plane. */
  rows: ErasureRows;
  /** Holds personas in memory; drops an owner's with their secrets. */
  personas: { forgetOwners(owners: Set<string>): void };
  /** Holds key settings in memory; drops an owner's here and in storage. */
  settings: { forgetOwners(owners: Set<string>): Promise<void> };
  /** Holds usage counters in memory; drops an owner's. */
  usage: { forget(owners: Set<string>): void };
  /** Saved gateway profiles on this replica's disk. */
  profiles: { removeOwners(owners: Set<string>): Promise<number> };
  /** Marks a project its owner holds deleted, stopping its browsers. */
  deleteProject(userId: string, id: string): Promise<unknown>;
  /** Writes one audit row. */
  audit(entry: Record<string, unknown>): void;
  /** Now, in ms; tests pass a clock. */
  now?: () => number;
};

/** Sessions that have ended and need nothing more from the project. */
const ENDED = new Set(['stopped', 'failed']);

/** Whether a subscription still bills. */
const billing = (sub) => !!sub && BILLING_STATES.includes(String(sub.status));

/** Erases deleted projects and deletes accounts. */
export class Erasure {
  /** Collaborators wired in by the composition root. */
  declare private readonly deps: ErasureDeps;

  /** Erasure over `deps`. */
  constructor(deps: ErasureDeps) {
    this.deps = deps;
  }

  /** Now, in ms. */
  private now() {
    return (this.deps.now ?? Date.now)();
  }

  /** One maintenance pass: forget past-grace projects on this replica, then erase the settled ones. */
  async maintain() {
    const due = ((await this.deps.store().list('project')) as Project[]).filter((p) => this.pastGrace(p));
    if (!due.length) return;
    await this.forgetLocally(new Set(due.map((p) => p.legacyOwner)));
    for (const p of due.filter((x) => this.settled(x))) await this.purge(p);
  }

  /** Deleted, not yet erased, and past its grace period. */
  private pastGrace(p: Project) {
    return !!p.deletedAt && !p.purgedAt && p.deletedAt + PURGE_GRACE_MS <= this.now();
  }

  /** Past grace long enough that every replica has forgotten it. */
  private settled(p: Project) {
    return p.deletedAt + PURGE_GRACE_MS + PURGE_SETTLE_MS <= this.now();
  }

  /** Drops what this replica holds for these owners: personas, settings, usage counters, saved profiles. */
  private async forgetLocally(owners: Set<string>) {
    this.deps.personas.forgetOwners(owners);
    this.deps.usage.forget(owners);
    await this.deps.settings.forgetOwners(owners);
    await this.deps.profiles.removeOwners(owners);
  }

  /**
   * Erases one project from storage and the control plane, then cuts its row
   * to a tombstone. Waits while a browser still needs cleaning up or a
   * recording is still archived; every step can run again after a crash.
   */
  async purge(p: Project) {
    if (await this.waiting(p)) return;
    await this.deps.rows.deleteProjectRows(p.legacyOwner, p.id);
    for (const kind of PROJECT_KINDS) await this.deleteKind(kind, p.id);
    await this.tombstone(p.id);
    this.deps.audit({ action: 'project.purge', targetType: 'project', targetId: p.id });
  }

  /** Whether a session still needs the project, or the recorder has not yet erased its recordings. */
  private async waiting(p: Project) {
    const [sessions, recordings] = await this.deps.store().load([
      { kind: 'session', project: p.id },
      { kind: 'recording', states: [p.legacyOwner] },
    ]);
    return recordings.length > 0 || sessions.some((r) => !ENDED.has(r.body.state));
  }

  /** Deletes every row of one kind the project holds. */
  private async deleteKind(kind: string, project: string) {
    const [rows] = await this.deps.store().load([{ kind, project }]);
    if (!rows.length) return;
    await this.deps.store().transact(async (tx) => {
      for (const r of rows) await tx.delete(kind, r.id);
    });
  }

  /** Keeps only what refuses the key and lets replicas match leftovers: id, owner fingerprint, when. */
  private async tombstone(id: string) {
    const purgedAt = this.now();
    await this.deps.store().transact(async (tx) => {
      const p = await tx.get('project', id);
      if (p) tx.put('project', id, { id, legacyOwner: p.legacyOwner, deletedAt: p.deletedAt, purgedAt, settings: {} });
    });
  }

  /**
   * Deletes a person's account: every project they own (erased after the
   * grace period), their memberships, keys, profile and sign-in. Refused
   * through "Login as", and while a subscription still bills.
   */
  async deleteAccount(userId: string, impersonatedBy?: string | null) {
    await this.assertDeletable(userId, impersonatedBy);
    const owned = await this.ownedProjects(userId);
    for (const id of owned) await this.deps.deleteProject(userId, id);
    await this.leaveProjects(userId);
    await this.deps.rows.deleteUserRows(userId);
    await this.deps.rows.deleteSignIn(userId);
    return { ok: true, projects: owned.length };
  }

  /** Refuses (403) a deletion through "Login as", and (409) one while a subscription still bills. */
  private async assertDeletable(userId: string, impersonatedBy?: string | null) {
    if (impersonatedBy) throw new HttpError(Status.FORBIDDEN, 'An account cannot be deleted through Login as');
    if (billing(await this.deps.rows.subscriptionOf(userId)))
      throw new HttpError(Status.CONFLICT, 'Cancel the subscription before deleting the account');
  }

  /** Ids of the live projects a person owns. */
  private async ownedProjects(userId: string) {
    const rows = (await this.deps.store().list('project', { states: [userId] })) as Project[];
    return rows.filter((p) => !p.deletedAt).map((p) => p.id);
  }

  /** Removes the person from projects others own. */
  private async leaveProjects(userId: string) {
    const [rows] = await this.deps.store().load([{ kind: 'membership', states: [userId] }]);
    if (!rows.length) return;
    await this.deps.store().transact(async (tx) => {
      for (const r of rows) await tx.delete('membership', r.id);
    });
  }
}
