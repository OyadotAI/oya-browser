/** Target compatibility refuses unsupported filters and renderer-pausing behavior explicitly. */
import type { NativeCommand } from './types.ts';
/** Validate the exact page-only auto-attachment subset. */
export function validateTargets(command: NativeCommand): void {
  validateContexts(command);
  if (command.method === 'Target.setDiscoverTargets' && typeof command.params.discover !== 'boolean')
    throw Error('discover must be a boolean');
  if (command.method !== 'Target.setAutoAttach') return;
  validateAutomatic(command.params);
}
/** Automatic sessions never promise unsupported paused renderers or worker discovery. */
function validateAutomatic(params: Record<string, unknown>): void {
  const { autoAttach, flatten, waitForDebuggerOnStart, filter } = params;
  if (typeof autoAttach !== 'boolean' || flatten !== true || waitForDebuggerOnStart !== false)
    throw Error(
      'Native automatic attachment requires autoAttach boolean, flatten:true and waitForDebuggerOnStart:false',
    );
  if (autoAttach || filter !== undefined) requirePageFilter(filter);
}
/** Requiring an explicit page-only filter prevents clients from assuming workers or iframe targets were attached. */
function requirePageFilter(filter: unknown): void {
  if (!Array.isArray(filter) || filter.length !== 1)
    throw Error('Native automatic attachment requires a page-only filter');
  const entry = filter[0];
  if (!entry || entry.type !== 'page' || (entry.exclude !== undefined && entry.exclude !== false))
    throw Error('Only page target inclusion is supported');
  if (Object.keys(entry).some((key) => !['type', 'exclude'].includes(key)))
    throw Error('Unsupported target filter option');
}

/** Ephemeral contexts cannot survive a disconnected owner or silently override proxy policy. */
function validateContexts({ method, params }: NativeCommand): void {
  if (params.browserContextId !== undefined && !params.browserContextId) throw Error('Empty browser context');
  if (method === 'Target.createBrowserContext' && params.disposeOnDetach !== true)
    throw Error('Native browser contexts require disposeOnDetach:true');
  if (method === 'Target.disposeBrowserContext' && typeof params.browserContextId !== 'string')
    throw Error('browserContextId is required');
}
