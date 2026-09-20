/**
 * Per-persona concurrency. One device cannot be in a thousand places at once,
 * so each persona has a cap on the browsers running as it, and a start past
 * the cap is refused rather than silently allowed.
 */
import { DEFAULT_MAX_CONCURRENT, type Persona } from './model.ts';
import type { PersonaDeps } from './service.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';

/** The browsers running as each persona, reported to metrics as they change. */
export class PersonaSlots {
  /** Gauges and counters the slots report into. */
  declare private readonly metrics: PersonaDeps['metrics'];
  /** id -> browser ids currently running as it. */
  declare private readonly active: Map<string, Set<string>>;

  /** No browsers running yet. */
  constructor(metrics: PersonaDeps['metrics']) {
    this.metrics = metrics;
    this.active = new Map();
  }

  /** Takes a slot for `browserId`, or throws 429 when the persona is at its cap. */
  acquire(persona: Persona, browserId: string) {
    if (!this.active.has(persona.id)) this.active.set(persona.id, new Set());
    const running = this.active.get(persona.id)!;
    const cap = persona.maxConcurrent ?? DEFAULT_MAX_CONCURRENT;
    if (!running.has(browserId) && running.size >= cap) this.refuse(persona, running.size, cap);
    running.add(browserId);
  }

  /** Counts the refusal and throws it. */
  private refuse(persona: Persona, running: number, cap: number): never {
    this.metrics.personaCapped.inc({});
    throw new HttpError(
      Status.TOO_MANY_REQUESTS,
      `Persona "${persona.name}" already has ${running} of ${cap} browsers running`,
    );
  }

  /** Frees the browser's slot. */
  release(persona: Persona | null | undefined, browserId: string) {
    const running = persona && this.active.get(persona.id);
    if (!running) return;
    running.delete(browserId);
    if (!running.size) this.active.delete(persona.id);
    this.report();
  }

  /** Browsers running as this persona now. */
  count(id: string) {
    return this.active.get(id)?.size || 0;
  }

  /** Forgets a persona's slots. */
  delete(id: string) {
    this.active.delete(id);
  }

  /** Forgets every slot. */
  clear() {
    this.active.clear();
  }

  /** Publishes the total of running persona browsers to metrics. */
  report() {
    this.metrics.personasActive.set(
      {},
      [...this.active.values()].reduce((n, s) => n + s.size, 0),
    );
  }
}
