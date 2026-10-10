/** Worker protection is installed by the owning native session before any worker script runs. */
import type { AppServices } from '../app/services.ts';
/** Retain lifecycle integration without a debugging connection or target instrumentation. */
export class WorkerCoverage {
  /** Session protection owner used by pages and all worker types. */
  private readonly deps: Pick<AppServices, 'protection' | 'persona'>;
  /** The native owner replaces endpoint discovery and worker attachment. */
  constructor(deps: Pick<AppServices, 'protection' | 'persona'>, _userData?: () => string) {
    this.deps = deps;
  }
  /** Refuse startup until immutable native worker protection is installed. */
  async cover(): Promise<void> {
    this.deps.protection.assertSession(this.deps.persona.session());
  }
  /** Native protection lives for the session; stopping never exposes workers without it. */
  stop(): void {}
}
