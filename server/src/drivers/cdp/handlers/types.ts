/**
 * The shape every CDP action handler shares.
 */
import type { CDPDriver } from '../driver.ts';

/** What an action answers with. */
export interface ActionResult {
  /** Whether the action succeeded. */
  ok: boolean;
  /** What it returned. */
  data?: any;
  /** Why it failed. */
  error?: string;
}

/**
 * Runs one action on the driver's attached page. `remaining` is what is left
 * of the command's time budget, never less than a second.
 */
export type Handler = (driver: CDPDriver, params: any, remaining: () => number) => Promise<ActionResult | any>;
