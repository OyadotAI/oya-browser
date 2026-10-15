/**
 * Whether this app runs on a person's own desktop, not under a provider or in a
 * container: desktop-only behaviour (persona screen semantics, page translation)
 * reads the one answer here.
 */
import type { ConfigStore } from './config-store.ts';

/** True on a person's own desktop: no provider launched this browser, and it is not in Docker. */
export const desktopRuntime = (config: ConfigStore | undefined, env: NodeJS.ProcessEnv = process.env): boolean =>
  !config?.values.provider && !env.OYA_DOCKER;
