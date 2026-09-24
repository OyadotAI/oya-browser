/**
 * The in-memory persona table and its autosave. Every persona lives here while
 * the server runs; changes are marked dirty and written to the repository on a
 * timer, so a burst of changes is one write.
 */
import type { Persona } from './model.ts';
import type { PersonaRepository } from './repository.ts';
import { AUTOSAVE_MS } from './constants.ts';

/** Personas by id, saved to a repository when they change. */
export class PersonaStore {
  /** Where personas are loaded from and saved to. */
  declare private readonly repository: PersonaRepository;
  /** id -> persona. Personas are small and bounded by customer count. */
  declare private readonly personas: Map<string, Persona>;
  /** Set when personas changed since the last save. */
  declare private dirty: boolean;
  /** Ids deleted since the last save, removed from storage by the next one. */
  declare private readonly deleted: Set<string>;
  /** The autosave interval, once started. */
  declare private timer: ReturnType<typeof setInterval> | null;

  /** An empty table over `repository`; call restore() to load it. */
  constructor(repository: PersonaRepository) {
    this.repository = repository;
    this.personas = new Map();
    this.dirty = false;
    this.deleted = new Set();
    this.timer = null;
  }

  /** The persona with this id, whoever owns it. */
  get(id: string) {
    return this.personas.get(id);
  }

  /** Every persona held. */
  all() {
    return [...this.personas.values()];
  }

  /** Adds or replaces a persona and marks the table for saving. */
  put(p: Persona) {
    this.personas.set(p.id, p);
    this.dirty = true;
  }

  /** Drops a persona and marks the table for saving. */
  delete(id: string) {
    this.personas.delete(id);
    this.deleted.add(id);
    this.dirty = true;
  }

  /** A held persona was changed in place: save it on the next tick. */
  touch() {
    this.dirty = true;
  }

  /**
   * Loads saved personas at startup. A failure fails the start: running on an
   * empty table would hand out new devices in place of the stored ones.
   */
  async restore() {
    for (const p of await this.repository.loadAll()) this.personas.set(p.id, p);
    if (this.personas.size) console.log(`[personas] restored ${this.personas.size}`);
  }

  /** Saves every persona when something changed, retrying on the next tick if the save fails. */
  async flush() {
    if (!this.dirty) return;
    this.dirty = false;
    const deleted = [...this.deleted];
    this.deleted.clear();
    await this.write(deleted).catch((e) => this.requeue(deleted, e));
  }

  /** Stores every persona, then removes the deleted ones. */
  private async write(deleted: string[]) {
    await this.repository.saveAll(this.all());
    await this.repository.remove(deleted);
  }

  /** Keeps a failed save for the next tick, and says so. */
  private requeue(deleted: string[], e: Error) {
    for (const id of deleted) this.deleted.add(id);
    this.dirty = true;
    console.error('[personas] save failed:', e.message);
  }

  /** Write changes every `everyMs`. */
  startAutosave(everyMs = AUTOSAVE_MS) {
    this.timer = setInterval(() => this.flush().catch(() => {}), everyMs);
    this.timer.unref?.();
  }

  /** Stops autosave and writes out pending changes, for shutdown. */
  async drain() {
    if (this.timer) clearInterval(this.timer);
    await this.flush();
  }

  /** Forgets every persona without saving. */
  clear() {
    this.personas.clear();
    this.deleted.clear();
    this.dirty = false;
  }
}
