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
  /** The autosave interval, once started. */
  declare private timer: ReturnType<typeof setInterval> | null;

  /** An empty table over `repository`; call restore() to load it. */
  constructor(repository: PersonaRepository) {
    this.repository = repository;
    this.personas = new Map();
    this.dirty = false;
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
    this.dirty = true;
  }

  /** A held persona was changed in place: save it on the next tick. */
  touch() {
    this.dirty = true;
  }

  /** Loads saved personas at startup; a failure is logged, not thrown. */
  async restore() {
    try {
      for (const p of await this.repository.loadAll()) this.personas.set(p.id, p);
      if (this.personas.size) console.log(`[personas] restored ${this.personas.size}`);
    } catch (e) {
      console.error('[personas] restore failed:', (e as Error).message);
    }
  }

  /** Saves every persona when something changed, retrying on the next tick if the save fails. */
  async flush() {
    if (!this.dirty) return;
    this.dirty = false;
    try {
      await this.repository.saveAll(this.all());
    } catch (e) {
      this.dirty = true;
      console.error('[personas] save failed:', (e as Error).message);
    }
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
    this.dirty = false;
  }
}
