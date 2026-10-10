/** Run-owned exact node and frame selection, with explicit recorded-alternative repairs. */
import { randomUUID } from 'node:crypto';
import type { Candidate, Step } from '../../workflow/index.ts';
import type { DriverTab } from '../actions/driver.ts';
import type { NativeValidationDeps } from './native-validation.ts';
import type { TargetRead } from './native-target.ts';
import { NativeScope, workflowScope } from './native-scope.ts';
import { locatorScript } from './native-locator-script.ts';
import { nativeValue } from './native-preflight.ts';
import type { NativeSelection } from './native-target-operations.ts';
/** Stable public selection shape used by the run executor. */
export type Selection = NativeSelection;
/** Node registries are run-specific and discarded on completion, cancellation or failure. */
export class NativeTargets {
  /** Production composition dependencies. */ private readonly deps: NativeValidationDeps;
  /** Immutable substitutions. */ private readonly vars: Record<string, unknown>;
  /** One live node slot per visited frame; no page-global DOM marker. */ private readonly key =
    'oya-workflow-' + randomUUID();
  /** Native frame scopes whose isolated node slots must be released. */ private readonly scopes = new Map<
    object,
    NativeScope
  >();
  /** Bind node capabilities to the owning run. */
  constructor(deps: NativeValidationDeps, vars: Record<string, unknown>) {
    this.deps = deps;
    this.vars = vars;
  }
  /** Assertions never repair; disabling auto-heal uses only the primary recorded candidate. */
  private candidates(step: Step): Candidate[] {
    return this.deps.options.autoHeal === false || step.action.startsWith('assert_')
      ? step.candidates.slice(0, 1)
      : step.candidates;
  }
  /** Resolve every frame selector through native browser-owned identity. */
  async select(step: Step, tab: DriverTab): Promise<Selection | undefined> {
    if (!step.candidates.length) return;
    const scope = await workflowScope(
      this.deps.driver,
      tab,
      step.frames.map((s) => nativeValue(s, this.vars)),
    );
    this.scopes.set(scope.frame || tab.view, scope);
    return this.find(step, scope);
  }
  /** Retry only read-only target inspection; never repeat an input action. */
  private async find(step: Step, scope: NativeScope): Promise<Selection | undefined> {
    for (const candidate of this.candidates(step)) {
      const selected = await this.inspect(step, scope, candidate);
      if (!selected) continue;
      this.repair(step, candidate);
      return selected;
    }
  }
  /** Unique matching binds the actual native DOM node, not a later selector lookup. */
  private async inspect(step: Step, scope: NativeScope, candidate: Candidate): Promise<Selection | undefined> {
    const token = randomUUID(),
      key = this.key;
    const resolved = { ...candidate, value: nativeValue(candidate.value, this.vars) };
    const hidden = step.action === 'upload_file';
    const read = await scope.evaluate<TargetRead>(
      locatorScript({ candidate: resolved, key, token, el: step.el || {}, hidden }),
    );
    return read.count === 1 && read.recorded ? { scope, key, token, read } : undefined;
  }
  /** Preserve original placeholders and a complete repaired draft. */
  private repair(step: Step, candidate: Candidate): void {
    if (candidate === step.candidates[0]) return;
    const candidates = [candidate, ...step.candidates.filter((c) => c !== candidate)];
    const steps = this.deps.draft.steps.map((s) => (s.id === step.id ? { ...s, candidates } : s));
    const identity = { stepId: step.id, original: step.candidates[0], replacement: candidate };
    this.deps.event({ type: 'repair', ...identity, draft: { ...this.deps.draft, steps } });
  }
  /** Await all document-local cleanup before releasing the run's client ownership. */
  async dispose(): Promise<void> {
    await Promise.allSettled([...this.scopes.values()].map((scope) => scope.release(this.key)));
    this.scopes.clear();
  }
}
