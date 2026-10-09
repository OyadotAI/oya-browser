/** Main-world evaluation and object lifecycles backed by Oya's native per-frame V8 registry. */
import type { NativePage } from './page.ts';
import type { NativeEventSink } from './log-stream.ts';
import type { RuntimeContext, RuntimeReply, RuntimeFrame } from './runtime-types.ts';
import { RuntimeContexts } from './runtime-contexts.ts';
import { runtimeParameters, runtimeOperation } from './runtime-commands.ts';
import { RuntimeValues } from './runtime-values.ts';
import { RuntimeEvents } from './runtime-events.ts';
/** Already-validated external runtime arguments. */
type Params = Record<string, unknown>;
/** Independent native context and value namespace for one external connection. */
export class NativeRuntime {
  /** Value handles and context ids never belong to the global page driver. */
  private readonly contexts: RuntimeContexts;
  /** Handle routing retains exact native documents across child-frame calls. */
  private readonly values = new RuntimeValues();
  /** Share child-frame identities with the connection’s frame-tree service. */
  constructor(frameId?: (frame: RuntimeFrame) => string) {
    this.contexts = new RuntimeContexts(frameId);
  }
  /** Exceptions are correlated without pretending unknown source locations are known. */
  private exception = 0;
  /** Execute only on the exact tab's main-world document; context selectors are validated. */
  async execute(page: NativePage, target: string, op: string, params: Params): Promise<object> {
    if (op === 'dropGroup') return this.dropGroup(page, target, params);
    const owner = this.values.owner(target, params);
    const selected = owner?.context || this.contexts.selected(target, params);
    const context = await this.contexts.current(page, target, selected?.frame);
    if (selected && context.uniqueId !== selected.uniqueId) throw Error('Stale or foreign native execution context');
    this.contexts.validate(context, params);
    return this.run(context, op, params, owner?.group || '');
  }
  /** The native document token guards navigation between selection and execution. */
  private async run(context: RuntimeContext, op: string, params: Params, group: string): Promise<object> {
    if (op === 'context') return {};
    this.values.validate(context, params);
    const nativeOp = runtimeOperation(op, params);
    const reply = await this.contexts.call(context.frame, context.document, nativeOp, runtimeParameters(op, params));
    this.values.remember(context, String(params.objectGroup ?? group), reply);
    if (op === 'drop') this.values.release(context.target, params);
    return this.response(reply, context);
  }
  /** A group spans this socket's known documents on one target, never another tab. */
  private async dropGroup(page: NativePage, target: string, params: Params): Promise<object> {
    const tasks = this.contexts.forTarget(target).map(async (context) => {
      this.contexts.requireOwned(page, context.frame);
      await this.contexts.call(context.frame, context.document, 'dropGroup', runtimeParameters('dropGroup', params));
    });
    const results = await Promise.allSettled(tasks);
    this.values.release(target, params);
    const failure = results.find((result) => result.status === 'rejected');
    return groupResult(failure);
  }

  /** Context notifications come from actual native readiness and document identities. */
  watch(page: NativePage, target: string, allowed: () => boolean, emit: NativeEventSink): () => void {
    return new RuntimeEvents({ page, allowed, emit, current: () => this.contexts.all(page, target) }).start();
  }
  /** Preserve page exceptions separately from native cancellation and unsupported capabilities. */
  private response(reply: RuntimeReply, context: RuntimeContext): object {
    if (reply.exception) return exceptionReply(reply, context.id, ++this.exception);
    if (reply.properties) return { result: reply.properties };
    return reply.result ? { result: reply.result } : {};
  }
  /** Release this socket's native value groups; other agents' handles remain intact. */
  dispose(): void {
    this.contexts.dispose();
    this.values.clear();
  }
}
/** Page exceptions have a separate result channel; source locations remain unknown. */
function exceptionReply(reply: RuntimeReply, context: number, id: number): object {
  const exception = reply.exception!;
  const exceptionDetails = { ...exception, ...exceptionPosition(context, id) };
  return { result: reply.result, exceptionDetails };
}
/** Unknown source positions are not inferred from generated wrapper code. */
function exceptionPosition(context: number, id: number): object {
  return { exceptionId: id, executionContextId: context, lineNumber: 0, columnNumber: 0 };
}

/** Group cleanup waits for all native documents even when one has been replaced. */
function groupResult(failure: PromiseSettledResult<unknown> | undefined): object {
  if (failure?.status === 'rejected') throw failure.reason;
  return {};
}
